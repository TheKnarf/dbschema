#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub extern "C" fn __rust_probestack() {}

pub mod backends;
pub mod config;
pub mod frontend;
pub mod ir;
pub mod lint;
pub mod passes;
pub mod postgres;
pub mod prisma;
pub mod test_runner;

use anyhow::Result;
// Keep types public via re-exports
use std::path::Path;

// Public re-exports
use crate::frontend::env::EnvVars;
pub use ir::{
    AggregateSpec, CollationSpec, CompositeTypeSpec, Config, DomainSpec, EnumSpec, EventTriggerSpec,
    ExtensionSpec, FunctionSpec, GrantSpec, MaterializedViewSpec, OutputSpec, PolicySpec, RoleSpec,
    SchemaSpec, SequenceSpec, TableSpec, TriggerSpec, ViewSpec,
};

// Loader abstraction: lets callers control how files are read.
pub trait Loader {
    fn load(&self, path: &Path) -> Result<String>;
}

// Pure API: parse + evaluate HCL config starting at `root_path` using a Loader.
pub fn load_config(root_path: &Path, loader: &dyn Loader, env: EnvVars) -> Result<Config> {
    frontend::load_root_with_loader(root_path, loader, env)
}

// Pure validation: check references etc.
pub fn validate(cfg: &Config, strict: bool) -> Result<()> {
    passes::validate(cfg, strict)
}

fn filter_config_with<F>(cfg: &Config, predicate: F) -> Config
where
    F: Fn(crate::config::ResourceKind) -> bool,
{
    use crate::config::ResourceKind as R;

    macro_rules! maybe {
        ($kind:ident, $field:ident) => {
            predicate(R::$kind)
                .then(|| cfg.$field.clone())
                .unwrap_or_default()
        };
    }

    Config {
        functions: maybe!(Functions, functions),
        aggregates: maybe!(Aggregates, aggregates),
        triggers: maybe!(Triggers, triggers),
        event_triggers: maybe!(EventTriggers, event_triggers),
        extensions: maybe!(Extensions, extensions),
        collations: maybe!(Collations, collations),
        sequences: maybe!(Sequences, sequences),
        schemas: maybe!(Schemas, schemas),
        enums: maybe!(Enums, enums),
        domains: maybe!(Domains, domains),
        types: maybe!(Types, types),
        tables: maybe!(Tables, tables),
        views: maybe!(Views, views),
        materialized: maybe!(Materialized, materialized),
        policies: maybe!(Policies, policies),
        roles: maybe!(Roles, roles),
        grants: maybe!(Grants, grants),
        tests: maybe!(Tests, tests),
        outputs: cfg.outputs.clone(),
        ..Default::default()
    }
}

/// Apply filters to a configuration based on target settings
pub fn apply_filters(
    cfg: &Config,
    include: &std::collections::HashSet<crate::config::ResourceKind>,
    exclude: &std::collections::HashSet<crate::config::ResourceKind>,
) -> Config {
    filter_config_with(cfg, |kind| {
        include.contains(&kind) && !exclude.contains(&kind)
    })
}

/// Apply resource filters to a configuration (string-based for TOML config)
pub fn apply_resource_filters(cfg: &Config, include: &[String], exclude: &[String]) -> Config {
    use std::collections::HashSet;

    // If no filters specified, include everything
    let include_all = include.is_empty() && exclude.is_empty();

    // Convert filter lists to sets for efficient lookup
    let include_set: HashSet<String> = include.iter().cloned().collect();
    let exclude_set: HashSet<String> = exclude.iter().cloned().collect();

    filter_config_with(cfg, |kind| {
        let resource_type = kind.to_string();
        if include_all {
            true
        } else if !include_set.is_empty() {
            include_set.contains(resource_type.as_str())
        } else {
            !exclude_set.contains(resource_type.as_str())
        }
    })
}

pub fn generate_with_backend(backend: &str, cfg: &Config, strict: bool) -> Result<String> {
    let be = backends::get_backend(backend)
        .ok_or_else(|| anyhow::anyhow!(format!("unknown backend '{backend}'")))?;
    be.generate(cfg, strict)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::env::EnvVars;
    use std::collections::HashMap;
    use std::path::PathBuf;

    struct MapLoader {
        files: HashMap<PathBuf, String>,
    }
    impl Loader for MapLoader {
        fn load(&self, path: &Path) -> Result<String> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing file: {}", path.display()))
        }
    }

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn parse_simple_function_and_trigger() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            function "set_updated_at" {
              schema = "public"
              language = "plpgsql"
              returns  = "trigger"
              body = <<-SQL
                BEGIN
                  NEW.updated_at = now();
                  RETURN NEW;
                END;
              SQL
            }
            trigger "users_upd" {
              schema = "public"
              table = "users"
              function = "set_updated_at"
              events = ["UPDATE"]
            }
            "#
            .to_string(),
        );

        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.functions.len(), 1);
        assert_eq!(cfg.triggers.len(), 1);
        validate(&cfg, false).unwrap();
        let sql = generate_with_backend("postgres", &cfg, false).unwrap();
        assert!(sql.contains("CREATE OR REPLACE FUNCTION \"public\".\"set_updated_at\""));
        assert!(sql.contains("CREATE TRIGGER \"users_upd\""));
    }

    #[test]
    fn parse_simple_event_trigger() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            function "ddl_logger" {
              language = "plpgsql"
              returns = "event_trigger"
              body = "BEGIN END;"
            }
            event_trigger "log_ddl" {
              event = "ddl_command_start"
              function = "ddl_logger"
            }
            "#
            .to_string(),
        );

        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.event_triggers.len(), 1);
        validate(&cfg, false).unwrap();
        let sql = generate_with_backend("postgres", &cfg, false).unwrap();
        assert!(sql.contains("CREATE EVENT TRIGGER"));
    }

    #[test]
    fn parse_with_module_and_vars() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            variable "schema" { default = "public" }
            variable "tables" { default = ["users", "orders"] }
            module "mod1" {
              source = "/root/mod"
              schema = var.schema
              for_each = var.tables
              table = each.value
            }
            "#
            .to_string(),
        );
        files.insert(
            p("/root/mod/main.hcl"),
            r#"
            variable "schema" { default = "public" }
            variable "table" {}
            function "f" {
              schema = var.schema
              language = "plpgsql"
              returns = "trigger"
              body = "BEGIN NEW.updated_at = now(); RETURN NEW; END;"
            }
            trigger "t" {
              schema = var.schema
              table = var.table
              function = "f"
              events = ["INSERT"]
            }
            "#
            .to_string(),
        );

        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert!(cfg.functions.len() >= 1);
        assert_eq!(cfg.triggers.len(), 2);
        validate(&cfg, false).unwrap();
    }

    #[test]
    fn module_outputs_resolved_and_printed() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            module "child" { source = "/root/mod" }
            output "answer" { value = module.child.value }
            "#
            .to_string(),
        );
        files.insert(
            p("/root/mod/main.hcl"),
            r#"
            output "value" { value = 42 }
            "#
            .to_string(),
        );

        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.outputs.len(), 1);
        assert_eq!(cfg.outputs[0].name, "answer");
        assert_eq!(
            cfg.outputs[0].value,
            hcl::Value::Number(hcl::Number::from(42))
        );
    }

    #[test]
    fn variable_type_and_validation() {
        use hcl::Value;
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            variable "count" {
              type = "number"
              validation {
                condition = var.count > 0
                error_message = "count must be > 0"
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };

        // Wrong type
        let env = EnvVars {
            vars: HashMap::from([("count".into(), Value::String("x".into()))]),
            ..EnvVars::default()
        };
        let err = load_config(&p("/root/main.hcl"), &loader, env).unwrap_err();
        assert!(err.to_string().contains("expected type number"));

        // Fails validation
        let env = EnvVars {
            vars: HashMap::from([("count".into(), Value::from(0))]),
            ..EnvVars::default()
        };
        let err = load_config(&p("/root/main.hcl"), &loader, env).unwrap_err();
        assert!(err.to_string().contains("count must be > 0"));

        // Passes
        let env = EnvVars {
            vars: HashMap::from([("count".into(), Value::from(2))]),
            ..EnvVars::default()
        };
        load_config(&p("/root/main.hcl"), &loader, env).unwrap();
    }

    #[test]
    fn for_each_array_in_trigger_uses_each_value() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            variable "schema" { default = "public" }
            variable "tables" { default = ["users", "orders"] }

            function "f" {
              schema = var.schema
              language = "plpgsql"
              returns  = "trigger"
              body = "BEGIN RETURN NEW; END;"
            }

            trigger "upd" {
              schema = var.schema
              for_each = var.tables
              table = each.value
              function = "f"
              events = ["UPDATE"]
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.triggers.len(), 2);
        validate(&cfg, false).unwrap();
        let sql = generate_with_backend("postgres", &cfg, false).unwrap();
        assert!(sql.contains("\"users\""));
        assert!(sql.contains("\"orders\""));
    }

    #[test]
    fn count_creates_multiple_triggers() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            function "f" {
              schema = "public"
              language = "plpgsql"
              returns  = "trigger"
              body = "BEGIN RETURN NEW; END;"
            }

            trigger "t" {
              count = 2
              schema = "public"
              table = "users"
              function = "f"
              events = ["INSERT"]
              name = "t_${count.index}"
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.triggers.len(), 2);
    }

    #[test]
    fn dynamic_block_expands_columns() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            variable "cols" {
              default = {
                id = { type = "serial", nullable = false },
                name = { type = "text", nullable = true }
              }
            }

            table "users" {
              dynamic "column" {
                for_each = var.cols
                labels   = [each.key]
                content {
                  type = each.value.type
                  nullable = each.value.nullable
                }
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.tables.len(), 1);
        let cols = &cfg.tables[0].columns;
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0].name, "id");
        assert!(!cols[0].nullable);
        assert_eq!(cols[1].name, "name");
        assert!(cols[1].nullable);
    }

    #[test]
    fn parse_extension_and_generate_sql() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            extension "pgcrypto" {}
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.extensions.len(), 1);
        let sql = generate_with_backend("postgres", &cfg, false).unwrap();
        assert!(sql.contains("CREATE EXTENSION IF NOT EXISTS \"pgcrypto\";"));
    }

    #[test]
    fn generate_json_backend() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            function "f" {
              schema = "public"
              language = "plpgsql"
              returns  = "trigger"
              body = "BEGIN RETURN NEW; END;"
            }
            trigger "t" {
              schema = "public"
              table = "users"
              timing = "BEFORE"
              events = ["UPDATE"]
              level  = "ROW"
              function = "f"
            }
            extension "pgcrypto" {}
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        let json = crate::generate_with_backend("json", &cfg, false).unwrap();
        assert!(json.contains("\"backend\": \"json\""));
        assert!(json.contains("\"functions\""));
        assert!(json.contains("\"triggers\""));
        assert!(json.contains("\"extensions\""));
    }

    #[test]
    fn parse_view_and_generate_sql_and_json() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            view "v_users" {
              schema = "public"
              replace = true
              sql = "SELECT 1 as x"
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.views.len(), 1);
        let sql = generate_with_backend("postgres", &cfg, false).unwrap();
        assert!(sql.contains("CREATE OR REPLACE VIEW \"public\".\"v_users\" AS"));
        let json = crate::generate_with_backend("json", &cfg, false).unwrap();
        assert!(json.contains("\"views\""));
    }

    #[test]
    fn parse_materialized_and_generate_sql_and_json() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            materialized "mv" {
              schema = "public"
              with_data = false
              sql = "SELECT 42 as x"
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.materialized.len(), 1);
        let sql = generate_with_backend("postgres", &cfg, false).unwrap();
        assert!(sql.contains("CREATE MATERIALIZED VIEW \"public\".\"mv\" AS"));
        assert!(sql.contains("WITH NO DATA"));
        let json = crate::generate_with_backend("json", &cfg, false).unwrap();
        assert!(json.contains("\"materialized\""));
    }

    #[test]
    fn parse_enum_and_generate_sql_json_prisma() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            enum "status" { values = ["active", "disabled"] }
            table "users" {
              column "id" {
                type = "serial"
                nullable = false
              }
              column "status" {
                type = "status"
                nullable = false
              }
              primary_key { columns = ["id"] }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.enums.len(), 1);
        let sql = generate_with_backend("postgres", &cfg, false).unwrap();
        assert!(sql.contains("CREATE TYPE \"public\".\"status\" AS ENUM"));
        let json = crate::generate_with_backend("json", &cfg, false).unwrap();
        assert!(json.contains("\"enums\""));
        let prisma = crate::generate_with_backend("prisma", &cfg, false).unwrap();
        assert!(prisma.contains("enum status"));
        assert!(prisma.contains("status status"));
    }

    #[test]
    fn parse_table_and_generate_sql() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            table "users" {
              schema = "public"
              column "id" {
                type = "serial"
                nullable = false
              }
              column "email" {
                type = "text"
                nullable = false
              }
              primary_key { columns = ["id"] }
              unique "users_email_key" { columns = ["email"] }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.tables.len(), 1);
        let sql = generate_with_backend("postgres", &cfg, false).unwrap();
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS \"public\".\"users\""));
        assert!(sql.contains("CREATE UNIQUE INDEX IF NOT EXISTS \"users_email_key\" ON \"public\".\"users\" (\"email\");"));
    }

    #[test]
    fn parse_policy_and_generate_sql_and_json() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            table "users" {
              schema = "public"
              column "id" {
                type = "serial"
                nullable = false
              }
              primary_key { columns = ["id"] }
            }
            policy "p_users_select" {
              schema = "public"
              table = "users"
              as = "permissive"
              command = "select"
              roles = ["app_user"]
              using = "true"
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.policies.len(), 1);
        let sql = generate_with_backend("postgres", &cfg, false).unwrap();
        assert!(sql.contains("CREATE POLICY \"p_users_select\" ON \"public\".\"users\""));
        let json = crate::generate_with_backend("json", &cfg, false).unwrap();
        assert!(json.contains("\"policies\""));
        assert!(json.contains("\"p_users_select\""));
    }

    #[test]
    fn apply_filters_excludes_resources() {
        use crate::config::ResourceKind as R;

        let cfg = Config {
            functions: vec![FunctionSpec {
                name: "f".into(),
                alt_name: None,
                schema: None,
                language: "sql".into(),
                parameters: vec![],
                returns: "void".into(),
                replace: false,
                volatility: None,
                strict: false,
                security: None,
                cost: None,
                body: String::new(),
                comment: None,
            }],
            tables: vec![TableSpec {
                name: "t".into(),
                alt_name: None,
                schema: None,
                if_not_exists: false,
                columns: vec![],
                primary_key: None,
                indexes: vec![],
                checks: vec![],
                foreign_keys: vec![],
                partition_by: None,
                partitions: vec![],
                back_references: vec![],
                lint_ignore: vec![],
                comment: None,
                map: None,
            }],
            ..Default::default()
        };

        let include: std::collections::HashSet<R> =
            vec![R::Functions, R::Tables].into_iter().collect();
        let exclude: std::collections::HashSet<R> = vec![R::Functions].into_iter().collect();

        let filtered = apply_filters(&cfg, &include, &exclude);
        assert_eq!(filtered.functions.len(), 0);
        assert_eq!(filtered.tables.len(), 1);
    }

    #[test]
    fn apply_resource_filters_handles_strings() {
        let cfg = Config {
            functions: vec![FunctionSpec {
                name: "f".into(),
                alt_name: None,
                schema: None,
                language: "sql".into(),
                parameters: vec![],
                returns: "void".into(),
                replace: false,
                volatility: None,
                strict: false,
                security: None,
                cost: None,
                body: String::new(),
                comment: None,
            }],
            tables: vec![TableSpec {
                name: "t".into(),
                alt_name: None,
                schema: None,
                if_not_exists: false,
                columns: vec![],
                primary_key: None,
                indexes: vec![],
                checks: vec![],
                foreign_keys: vec![],
                partition_by: None,
                partitions: vec![],
                back_references: vec![],
                lint_ignore: vec![],
                comment: None,
                map: None,
            }],
            ..Default::default()
        };

        let filtered = apply_resource_filters(&cfg, &vec!["tables".into()], &[]);
        assert_eq!(filtered.functions.len(), 0);
        assert_eq!(filtered.tables.len(), 1);
    }

    #[test]
    fn parse_role_and_grant() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            role "app" {
              login    = true
              createdb = true
            }
            grant "g" {
              role       = "app"
              privileges = ["ALL"]
              database   = "appdb"
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.roles.len(), 1);
        assert_eq!(cfg.grants.len(), 1);
        let sql = generate_with_backend("postgres", &cfg, false).unwrap();
        assert!(sql.contains("CREATE ROLE \"app\" LOGIN CREATEDB;"));
        assert!(sql.contains("GRANT ALL PRIVILEGES ON DATABASE \"appdb\" TO \"app\";"));
    }
}
