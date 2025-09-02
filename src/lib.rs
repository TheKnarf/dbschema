pub mod backends;
pub mod config;
pub mod frontend;
pub mod ir;
pub mod test_runner;
pub mod passes;

use anyhow::Result;
// Keep types public via re-exports
use std::path::Path;

// Public re-exports
pub use ir::{
    Config, EnumSpec, EnvVars, ExtensionSpec, FunctionSpec, MaterializedViewSpec, PolicySpec,
    SchemaSpec, TableSpec, TriggerSpec, ViewSpec,
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

    Config {
        functions: if predicate(R::Functions) {
            cfg.functions.clone()
        } else {
            Vec::new()
        },
        triggers: if predicate(R::Triggers) {
            cfg.triggers.clone()
        } else {
            Vec::new()
        },
        extensions: if predicate(R::Extensions) {
            cfg.extensions.clone()
        } else {
            Vec::new()
        },
        schemas: if predicate(R::Schemas) {
            cfg.schemas.clone()
        } else {
            Vec::new()
        },
        enums: if predicate(R::Enums) {
            cfg.enums.clone()
        } else {
            Vec::new()
        },
        tables: if predicate(R::Tables) {
            cfg.tables.clone()
        } else {
            Vec::new()
        },
        views: if predicate(R::Views) {
            cfg.views.clone()
        } else {
            Vec::new()
        },
        materialized: if predicate(R::Materialized) {
            cfg.materialized.clone()
        } else {
            Vec::new()
        },
        policies: if predicate(R::Policies) {
            cfg.policies.clone()
        } else {
            Vec::new()
        },
        tests: if predicate(R::Tests) {
            cfg.tests.clone()
        } else {
            Vec::new()
        },
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
        let resource_type = kind.as_str();
        if include_all {
            true
        } else if !include_set.is_empty() {
            include_set.contains(resource_type)
        } else {
            !exclude_set.contains(resource_type)
        }
    })
}

// Pure SQL generation wrapper - uses postgres backend by default
pub fn generate_sql(cfg: &Config) -> Result<String> {
    backends::postgres::to_sql(cfg)
}

pub fn generate_with_backend(
    backend: &str,
    cfg: &Config,
    env: &EnvVars,
    strict: bool,
) -> Result<String> {
    let be = backends::get_backend(backend)
        .ok_or_else(|| anyhow::anyhow!(format!("unknown backend '{backend}'")))?;
    be.generate(cfg, env, strict)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let sql = generate_sql(&cfg).unwrap();
        assert!(sql.contains("CREATE OR REPLACE FUNCTION \"public\".\"set_updated_at\""));
        assert!(sql.contains("CREATE TRIGGER \"users_upd\""));
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
        let sql = generate_sql(&cfg).unwrap();
        assert!(sql.contains("\"users\""));
        assert!(sql.contains("\"orders\""));
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
        let sql = generate_sql(&cfg).unwrap();
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
        let env = EnvVars::default();
        let json = crate::generate_with_backend("json", &cfg, &env, false).unwrap();
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
        let sql = generate_sql(&cfg).unwrap();
        assert!(sql.contains("CREATE OR REPLACE VIEW \"public\".\"v_users\" AS"));
        let env = EnvVars::default();
        let json = crate::generate_with_backend("json", &cfg, &env, false).unwrap();
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
        let sql = generate_sql(&cfg).unwrap();
        assert!(sql.contains("CREATE MATERIALIZED VIEW \"public\".\"mv\" AS"));
        assert!(sql.contains("WITH NO DATA"));
        let env = EnvVars::default();
        let json = crate::generate_with_backend("json", &cfg, &env, false).unwrap();
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
        let sql = generate_sql(&cfg).unwrap();
        assert!(sql.contains("CREATE TYPE \"public\".\"status\" AS ENUM"));
        let env = EnvVars::default();
        let json = crate::generate_with_backend("json", &cfg, &env, false).unwrap();
        assert!(json.contains("\"enums\""));
        let env = EnvVars::default();
        let prisma = crate::generate_with_backend("prisma", &cfg, &env, false).unwrap();
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
        let sql = generate_sql(&cfg).unwrap();
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
        let sql = generate_sql(&cfg).unwrap();
        assert!(sql.contains("CREATE POLICY \"p_users_select\" ON \"public\".\"users\""));
        let env = EnvVars::default();
        let json = crate::generate_with_backend("json", &cfg, &env, false).unwrap();
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
                returns: "void".into(),
                replace: false,
                security_definer: false,
                body: String::new(),
            }],
            tables: vec![TableSpec {
                name: "t".into(),
                table_name: None,
                schema: None,
                if_not_exists: false,
                columns: vec![],
                primary_key: None,
                indexes: vec![],
                foreign_keys: vec![],
                back_references: vec![],
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
                returns: "void".into(),
                replace: false,
                security_definer: false,
                body: String::new(),
            }],
            tables: vec![TableSpec {
                name: "t".into(),
                table_name: None,
                schema: None,
                if_not_exists: false,
                columns: vec![],
                primary_key: None,
                indexes: vec![],
                foreign_keys: vec![],
                back_references: vec![],
            }],
            ..Default::default()
        };

        let filtered = apply_resource_filters(&cfg, &vec!["tables".into()], &[]);
        assert_eq!(filtered.functions.len(), 0);
        assert_eq!(filtered.tables.len(), 1);
    }
}
