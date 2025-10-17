#[cfg(target_arch = "x86_64")]
#[unsafe(no_mangle)]
pub extern "C" fn __rust_probestack() {}

pub mod backends;
pub mod config;
pub mod frontend;
pub mod ir;
pub mod lint;
pub mod passes;
pub mod postgres;
pub mod prisma;
pub mod provider;
pub mod test_runner;

use anyhow::Result;
// Keep types public via re-exports
use std::path::Path;

// Public re-exports
use crate::frontend::env::EnvVars;
pub use ir::{
    AggregateSpec, CollationSpec, CompositeTypeSpec, Config, DomainSpec, EnumSpec,
    EventTriggerSpec, ExtensionSpec, FunctionSpec, GrantSpec, MaterializedViewSpec, OutputSpec,
    PolicySpec, ProcedureSpec, RoleSpec, SchemaSpec, SequenceSpec, TableSpec, TablespaceSpec,
    TriggerSpec, ViewSpec,
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
        procedures: maybe!(Procedures, procedures),
        aggregates: maybe!(Aggregates, aggregates),
        operators: maybe!(Operators, operators),
        triggers: maybe!(Triggers, triggers),
        rules: maybe!(Rules, rules),
        event_triggers: maybe!(EventTriggers, event_triggers),
        extensions: maybe!(Extensions, extensions),
        collations: maybe!(Collations, collations),
        sequences: maybe!(Sequences, sequences),
        schemas: maybe!(Schemas, schemas),
        enums: maybe!(Enums, enums),
        domains: maybe!(Domains, domains),
        types: maybe!(Types, types),
        tables: maybe!(Tables, tables),
        indexes: maybe!(Indexes, indexes),
        statistics: maybe!(Statistics, statistics),
        views: maybe!(Views, views),
        materialized: maybe!(Materialized, materialized),
        policies: maybe!(Policies, policies),
        roles: maybe!(Roles, roles),
        tablespaces: maybe!(Tablespaces, tablespaces),
        grants: maybe!(Grants, grants),
        foreign_data_wrappers: maybe!(ForeignDataWrappers, foreign_data_wrappers),
        foreign_servers: maybe!(ForeignServers, foreign_servers),
        foreign_tables: maybe!(ForeignTables, foreign_tables),
        text_search_dictionaries: maybe!(TextSearchDictionaries, text_search_dictionaries),
        text_search_configurations: maybe!(TextSearchConfigurations, text_search_configurations),
        text_search_templates: maybe!(TextSearchTemplates, text_search_templates),
        text_search_parsers: maybe!(TextSearchParsers, text_search_parsers),
        publications: maybe!(Publications, publications),
        subscriptions: maybe!(Subscriptions, subscriptions),
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
    fn module_for_each_can_use_data_sources() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            data "prisma_schema" "app" {
              file = "/root/schema.prisma"
            }

            module "mirror" {
              source  = "/root/mod"
              for_each = data.prisma_schema.app.models
              name    = each.key
            }
            "#
            .to_string(),
        );
        files.insert(
            p("/root/mod/main.hcl"),
            r#"
            variable "name" {}

            table "mirror" {
              comment = var.name

              column "id" {
                type = "text"
              }
            }
            "#
            .to_string(),
        );
        files.insert(
            p("/root/schema.prisma"),
            r#"
            model User {
              id    Int @id
            }

            model Post {
              id    Int @id
            }
            "#
            .to_string(),
        );

        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();

        assert_eq!(cfg.tables.len(), 2);
        let mut comments: Vec<Option<String>> =
            cfg.tables.iter().map(|t| t.comment.clone()).collect();
        comments.sort();
        assert_eq!(
            comments,
            vec![Some("Post".to_string()), Some("User".to_string())]
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
    fn data_prisma_schema_exposes_models_and_enums() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            data "prisma_schema" "app" {
              file = "/root/schema.prisma"
            }

            table "audit_log" {
              schema = "public"

              column "user_id" {
                type     = "bigint"
                nullable = data.prisma_schema.app.models.User.fields.id.type.optional
                comment  = data.prisma_schema.app.models.User.fields.id.type.name
              }

              column "middle_name" {
                type     = "text"
                nullable = data.prisma_schema.app.models.User.fields.middleName.type.optional
                comment  = data.prisma_schema.app.models.User.fields.middleName.type.name
              }

              column "status" {
                type    = "text"
                comment = data.prisma_schema.app.enums.Status.values.ACTIVE.name
              }

              column "inactive_label" {
                type    = "text"
                comment = data.prisma_schema.app.enums.Status.values.INACTIVE.mapped_name
              }
            }
            "#
            .to_string(),
        );
        files.insert(
            p("/root/schema.prisma"),
            r#"
            model User {
              id          Int      @id @default(autoincrement())
              email       String   @unique
              middleName  String?
              status      Status   @default(ACTIVE)
            }

            enum Status {
              ACTIVE
              INACTIVE @map("inactive")
            }
            "#
            .to_string(),
        );

        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.tables.len(), 1);
        let table = &cfg.tables[0];
        assert_eq!(table.columns.len(), 4);
        assert_eq!(table.columns[0].nullable, false);
        assert_eq!(table.columns[0].comment.as_deref(), Some("Int"));
        assert_eq!(table.columns[1].nullable, true);
        assert_eq!(table.columns[1].comment.as_deref(), Some("String"));
        assert_eq!(table.columns[2].comment.as_deref(), Some("ACTIVE"));
        assert_eq!(table.columns[3].comment.as_deref(), Some("inactive"));
    }

    #[test]
    fn clone_prisma_table_with_dynamic_columns() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            data "prisma_schema" "source" {
              file = "/root/schema.prisma"
            }

            table "user_clone" {
              schema = "public"

              dynamic "column" {
                for_each = data.prisma_schema.source.models.User.fields
                labels   = [each.key]

                content {
                  type     = each.value.type.name == "Int" ? "integer" : each.value.type.name == "String" ? "text" : each.value.type.name == "DateTime" ? "timestamptz" : "text"
                  nullable = each.value.type.optional
                }
              }

              primary_key {
                columns = ["id"]
              }
            }
            "#
            .to_string(),
        );
        files.insert(
            p("/root/schema.prisma"),
            r#"
            model User {
              id        Int      @id @default(autoincrement())
              email     String   @unique
              name      String?
              createdAt DateTime @default(now())
              updatedAt DateTime @updatedAt
            }
            "#
            .to_string(),
        );

        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();

        assert_eq!(cfg.tables.len(), 1);
        let table = &cfg.tables[0];
        assert_eq!(table.name, "user_clone");

        // Verify all 5 columns were created from Prisma model
        assert_eq!(table.columns.len(), 5);

        // Verify column names match Prisma fields
        let col_names: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
        assert!(col_names.contains(&"id"));
        assert!(col_names.contains(&"email"));
        assert!(col_names.contains(&"name"));
        assert!(col_names.contains(&"createdAt"));
        assert!(col_names.contains(&"updatedAt"));

        // Verify types were mapped correctly
        let id_col = table.columns.iter().find(|c| c.name == "id").unwrap();
        assert_eq!(id_col.r#type, "integer");
        assert!(!id_col.nullable); // Int is not optional in Prisma

        let email_col = table.columns.iter().find(|c| c.name == "email").unwrap();
        assert_eq!(email_col.r#type, "text");
        assert!(!email_col.nullable); // String is not optional

        let name_col = table.columns.iter().find(|c| c.name == "name").unwrap();
        assert_eq!(name_col.r#type, "text");
        assert!(name_col.nullable); // String? is optional in Prisma

        let created_at_col = table.columns.iter().find(|c| c.name == "createdAt").unwrap();
        assert_eq!(created_at_col.r#type, "timestamptz");
        assert!(!created_at_col.nullable);

        let updated_at_col = table.columns.iter().find(|c| c.name == "updatedAt").unwrap();
        assert_eq!(updated_at_col.r#type, "timestamptz");
        assert!(!updated_at_col.nullable);

        // Verify primary key was created from Prisma @id attribute
        assert!(table.primary_key.is_some());
        let pk = table.primary_key.as_ref().unwrap();
        assert_eq!(pk.columns.len(), 1);
        assert_eq!(pk.columns[0], "id");

        validate(&cfg, false).unwrap();
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
    fn apply_filters_preserves_extended_resources() {
        use crate::config::ResourceKind as R;
        use crate::ir::{
            ColumnSpec, ForeignDataWrapperSpec, ForeignServerSpec, ForeignTableSpec,
            PublicationSpec, PublicationTableSpec, StandaloneIndexSpec, StatisticsSpec,
            SubscriptionSpec, TextSearchConfigurationMappingSpec, TextSearchConfigurationSpec,
            TextSearchDictionarySpec, TextSearchParserSpec, TextSearchTemplateSpec,
        };
        use std::collections::HashSet;

        let cfg = Config {
            indexes: vec![StandaloneIndexSpec {
                name: "idx".into(),
                table: "t".into(),
                schema: Some("public".into()),
                columns: vec!["col".into()],
                expressions: vec![],
                r#where: None,
                orders: vec![],
                operator_classes: vec![],
                unique: false,
            }],
            statistics: vec![StatisticsSpec {
                name: "stats".into(),
                alt_name: None,
                schema: Some("public".into()),
                table: "t".into(),
                columns: vec!["col".into()],
                kinds: vec![],
                comment: None,
            }],
            foreign_data_wrappers: vec![ForeignDataWrapperSpec {
                name: "fdw".into(),
                alt_name: None,
                handler: None,
                validator: None,
                options: vec![],
                comment: None,
            }],
            foreign_servers: vec![ForeignServerSpec {
                name: "server".into(),
                alt_name: None,
                wrapper: "fdw".into(),
                r#type: None,
                version: None,
                options: vec![],
                comment: None,
            }],
            foreign_tables: vec![ForeignTableSpec {
                name: "foreign_table".into(),
                alt_name: None,
                schema: Some("public".into()),
                server: "server".into(),
                columns: vec![ColumnSpec {
                    name: "col".into(),
                    r#type: "text".into(),
                    nullable: true,
                    default: None,
                    db_type: None,
                    lint_ignore: vec![],
                    comment: None,
                    count: 0,
                }],
                options: vec![],
                comment: None,
            }],
            text_search_dictionaries: vec![TextSearchDictionarySpec {
                name: "dict".into(),
                alt_name: None,
                schema: Some("public".into()),
                template: "simple".into(),
                options: vec![],
                comment: None,
            }],
            text_search_configurations: vec![TextSearchConfigurationSpec {
                name: "cfg".into(),
                alt_name: None,
                schema: Some("public".into()),
                parser: "default".into(),
                mappings: vec![TextSearchConfigurationMappingSpec {
                    tokens: vec!["asciiword".into()],
                    dictionaries: vec!["dict".into()],
                }],
                comment: None,
            }],
            text_search_templates: vec![TextSearchTemplateSpec {
                name: "tmpl".into(),
                alt_name: None,
                schema: Some("public".into()),
                init: None,
                lexize: "lexize".into(),
                comment: None,
            }],
            text_search_parsers: vec![TextSearchParserSpec {
                name: "parser".into(),
                alt_name: None,
                schema: Some("public".into()),
                start: "start".into(),
                gettoken: "get".into(),
                end: "end".into(),
                headline: None,
                lextypes: "lex".into(),
                comment: None,
            }],
            publications: vec![PublicationSpec {
                name: "pub".into(),
                alt_name: None,
                all_tables: false,
                tables: vec![PublicationTableSpec {
                    schema: Some("public".into()),
                    table: "t".into(),
                }],
                publish: vec!["insert".into()],
                comment: None,
            }],
            subscriptions: vec![SubscriptionSpec {
                name: "sub".into(),
                alt_name: None,
                connection: "dbname=app".into(),
                publications: vec!["pub".into()],
                comment: None,
            }],
            ..Default::default()
        };

        use crate::config::ResourceKind;

        let include_all = ResourceKind::default_include_set();
        let filtered = apply_filters(&cfg, &include_all, &HashSet::new());
        assert_eq!(filtered.indexes.len(), 1);
        assert_eq!(filtered.statistics.len(), 1);
        assert_eq!(filtered.foreign_data_wrappers.len(), 1);
        assert_eq!(filtered.foreign_servers.len(), 1);
        assert_eq!(filtered.foreign_tables.len(), 1);
        assert_eq!(filtered.text_search_dictionaries.len(), 1);
        assert_eq!(filtered.text_search_configurations.len(), 1);
        assert_eq!(filtered.text_search_templates.len(), 1);
        assert_eq!(filtered.text_search_parsers.len(), 1);
        assert_eq!(filtered.publications.len(), 1);
        assert_eq!(filtered.subscriptions.len(), 1);

        let mut only_indexes = HashSet::new();
        only_indexes.insert(R::Indexes);
        let filtered_only_indexes = apply_filters(&cfg, &only_indexes, &HashSet::new());
        assert_eq!(filtered_only_indexes.indexes.len(), 1);
        assert_eq!(filtered_only_indexes.statistics.len(), 0);
        assert_eq!(filtered_only_indexes.foreign_tables.len(), 0);

        let exclude_indexes: HashSet<R> = vec![R::Indexes].into_iter().collect();
        let filtered_without_indexes = apply_filters(&cfg, &include_all, &exclude_indexes);
        assert_eq!(filtered_without_indexes.indexes.len(), 0);
        assert_eq!(filtered_without_indexes.statistics.len(), 1);
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
