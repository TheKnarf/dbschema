#[cfg(target_arch = "x86_64")]
#[unsafe(no_mangle)]
pub extern "C" fn __rust_probestack() {}

pub mod backends;
pub mod config;
pub mod frontend;
pub mod ir;
pub mod lint;
pub mod passes;
pub mod prisma;
pub mod provider;
#[cfg(feature = "scenario")]
pub mod scenario;
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
        scenarios: cfg.scenarios.clone(),
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

        let created_at_col = table
            .columns
            .iter()
            .find(|c| c.name == "createdAt")
            .unwrap();
        assert_eq!(created_at_col.r#type, "timestamptz");
        assert!(!created_at_col.nullable);

        let updated_at_col = table
            .columns
            .iter()
            .find(|c| c.name == "updatedAt")
            .unwrap();
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
    fn prisma_back_reference_relations_have_names() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            table "blob" {
              schema = "public"
              column "blobId" {
                type = "text"
                nullable = false
              }
              primary_key { columns = ["blobId"] }
            }

            table "commit" {
              schema = "public"
              column "commitId" {
                type = "text"
                nullable = false
              }
              column "blobId" {
                type = "text"
                nullable = false
              }
              primary_key { columns = ["commitId"] }
              foreign_key {
                name = "blob_fk"
                columns = ["blobId"]
                ref {
                  schema = "public"
                  table = "blob"
                  columns = ["blobId"]
                }
                back_reference_name = "commits"
                on_delete = "RESTRICT"
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        let prisma = crate::generate_with_backend("prisma", &cfg, false).unwrap();
        assert!(
            prisma.contains("commits Commit[] @relation(name: \"commits\")"),
            "expected commits relation field to include relation name:\n{prisma}"
        );
        assert!(
            prisma.contains(
                "blob_fk Blob @relation(name: \"commits\", fields: [blobId], references: [blobId]"
            ),
            "expected blob relation field to reference relation name:\n{prisma}"
        );
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

    #[test]
    fn parse_assert_eq_in_test() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            test "eq_check" {
              assert_eq {
                query    = "SELECT 'hello'"
                expected = "hello"
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.tests.len(), 1);
        assert_eq!(cfg.tests[0].name, "eq_check");
        assert_eq!(cfg.tests[0].assert_eq.len(), 1);
        assert_eq!(cfg.tests[0].assert_eq[0].query, "SELECT 'hello'");
        assert_eq!(cfg.tests[0].assert_eq[0].expected, "hello");
    }

    #[test]
    fn parse_assert_error_in_test() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            test "error_check" {
              assert_error {
                sql              = "SELECT 1/0"
                message_contains = "division by zero"
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.tests.len(), 1);
        assert_eq!(cfg.tests[0].name, "error_check");
        assert_eq!(cfg.tests[0].assert_error.len(), 1);
        assert_eq!(cfg.tests[0].assert_error[0].sql, "SELECT 1/0");
        assert_eq!(cfg.tests[0].assert_error[0].message_contains, "division by zero");
    }

    #[test]
    fn parse_assert_notify_in_test() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            test "notify_check" {
              setup = ["SELECT pg_notify('my_channel', 'payload_data')"]
              assert_notify {
                channel          = "my_channel"
                payload_contains = "payload_data"
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.tests.len(), 1);
        assert_eq!(cfg.tests[0].name, "notify_check");
        assert_eq!(cfg.tests[0].assert_notify.len(), 1);
        assert_eq!(cfg.tests[0].assert_notify[0].channel, "my_channel");
        assert_eq!(cfg.tests[0].assert_notify[0].payload_contains, Some("payload_data".to_string()));
    }

    #[test]
    fn parse_assert_notify_without_payload() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            test "notify_no_payload" {
              setup = ["SELECT pg_notify('ch', '')"]
              assert_notify {
                channel = "ch"
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.tests[0].assert_notify[0].channel, "ch");
        assert_eq!(cfg.tests[0].assert_notify[0].payload_contains, None);
    }

    #[test]
    fn parse_multiple_assertion_types_in_one_test() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            test "mixed" {
              setup = ["SELECT 1"]
              assert = ["SELECT true"]
              assert_fail = ["SELECT 1 FROM nonexistent_table"]
              assert_eq {
                query    = "SELECT 'x'"
                expected = "x"
              }
              assert_error {
                sql              = "SELECT 1/0"
                message_contains = "division"
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        let t = &cfg.tests[0];
        assert_eq!(t.setup.len(), 1);
        assert_eq!(t.asserts.len(), 1);
        assert_eq!(t.assert_fail.len(), 1);
        assert_eq!(t.assert_eq.len(), 1);
        assert_eq!(t.assert_error.len(), 1);
    }

    #[test]
    fn test_requires_at_least_one_assertion() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            test "empty" {
              setup = ["SELECT 1"]
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let err = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap_err();
        assert!(err.to_string().contains("must define at least one assertion type"));
    }

    #[test]
    fn parse_assert_snapshot_in_test() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            table "users" {
              column "name" {
                type = "String"
              }
            }
            test "snapshot_check" {
              setup = [
                "INSERT INTO users (name) VALUES ('alice')",
              ]
              assert_snapshot {
                query = "SELECT name FROM users ORDER BY name"
                rows = [
                  ["alice"],
                ]
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.tests.len(), 1);
        assert_eq!(cfg.tests[0].name, "snapshot_check");
        assert_eq!(cfg.tests[0].assert_snapshot.len(), 1);
        assert_eq!(cfg.tests[0].assert_snapshot[0].query, "SELECT name FROM users ORDER BY name");
        assert_eq!(cfg.tests[0].assert_snapshot[0].rows, vec![vec!["alice".to_string()]]);
    }

    #[test]
    fn parse_assert_snapshot_multiple_rows_and_columns() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            test "multi_snap" {
              assert_snapshot {
                query = "SELECT 1 AS a, 2 AS b UNION ALL SELECT 3, 4"
                rows = [
                  ["1", "2"],
                  ["3", "4"],
                ]
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.tests[0].assert_snapshot[0].rows.len(), 2);
        assert_eq!(cfg.tests[0].assert_snapshot[0].rows[0], vec!["1", "2"]);
        assert_eq!(cfg.tests[0].assert_snapshot[0].rows[1], vec!["3", "4"]);
    }

    #[test]
    fn parse_invariant_block() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            invariant "positive_counts" {
              assert = [
                "SELECT 1 = 1",
                "SELECT 2 > 0",
              ]
            }
            test "dummy" {
              assert = ["SELECT true"]
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.invariants.len(), 1);
        assert_eq!(cfg.invariants[0].name, "positive_counts");
        assert_eq!(cfg.invariants[0].asserts.len(), 2);
        assert_eq!(cfg.invariants[0].asserts[0], "SELECT 1 = 1");
    }

    #[test]
    fn invariant_requires_at_least_one_assert() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            invariant "empty" {
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let err = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap_err();
        assert!(err.to_string().contains("must define at least one assert"));
    }

    #[test]
    fn test_for_each_array_generates_indexed_tests() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            test "check" {
              for_each = ["a", "b", "c"]
              assert = ["SELECT '${each.value}' IS NOT NULL"]
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.tests.len(), 3);
        assert_eq!(cfg.tests[0].name, "check[0]");
        assert_eq!(cfg.tests[1].name, "check[1]");
        assert_eq!(cfg.tests[2].name, "check[2]");
        assert!(cfg.tests[0].asserts[0].contains("'a'"));
        assert!(cfg.tests[1].asserts[0].contains("'b'"));
        assert!(cfg.tests[2].asserts[0].contains("'c'"));
    }

    #[test]
    fn test_for_each_object_generates_keyed_tests() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            test "status" {
              for_each = {
                low  = "rejected"
                high = "approved"
              }
              assert_eq {
                query    = "SELECT '${each.value}'"
                expected = each.value
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.tests.len(), 2);
        let names: Vec<&str> = cfg.tests.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"status[high]"));
        assert!(names.contains(&"status[low]"));
    }

    #[test]
    fn test_for_each_with_count() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            test "counted" {
              count = 3
              assert = ["SELECT ${count.index} >= 0"]
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.tests.len(), 3);
        assert_eq!(cfg.tests[0].name, "counted[0]");
        assert_eq!(cfg.tests[1].name, "counted[1]");
        assert_eq!(cfg.tests[2].name, "counted[2]");
    }

    #[test]
    fn parse_scenario_block() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            scenario "bid_processing" {
              program = <<-ASP
                bidder(alice; bob).
                amount(50; 100).
                1 { bid(B, A) : bidder(B), amount(A) } 2.
                :- bid(B, A1), bid(B, A2), A1 != A2.
              ASP

              setup = [
                "INSERT INTO items (name) VALUES ('Test')",
              ]

              map "bid" {
                sql = "INSERT INTO bids (bidder, amount) VALUES ('{1}', {2})"
              }

              runs = 5
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.scenarios.len(), 1);
        assert_eq!(cfg.scenarios[0].name, "bid_processing");
        assert!(cfg.scenarios[0].program.contains("bidder(alice; bob)"));
        assert_eq!(cfg.scenarios[0].setup.len(), 1);
        assert_eq!(cfg.scenarios[0].maps.len(), 1);
        assert_eq!(cfg.scenarios[0].maps[0].atom_name, "bid");
        assert!(cfg.scenarios[0].maps[0].sql.contains("{1}"));
        assert_eq!(cfg.scenarios[0].runs, 5);
    }

    #[test]
    fn parse_scenario_with_multiple_maps() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            scenario "complex" {
              program = "fact(a). action(b)."

              map "fact" {
                sql = "INSERT INTO facts VALUES ('{1}')"
              }

              map "action" {
                sql = "INSERT INTO actions VALUES ('{1}')"
              }

              runs = 0
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.scenarios.len(), 1);
        assert_eq!(cfg.scenarios[0].maps.len(), 2);
        assert_eq!(cfg.scenarios[0].maps[0].atom_name, "fact");
        assert_eq!(cfg.scenarios[0].maps[1].atom_name, "action");
        assert_eq!(cfg.scenarios[0].runs, 0);
    }

    #[test]
    fn parse_scenario_defaults_runs_to_zero() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            scenario "minimal" {
              program = "a(1)."
              map "a" {
                sql = "SELECT {1}"
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.scenarios[0].runs, 0);
    }

    #[test]
    fn parse_scenario_map_order_by() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            scenario "ordered" {
              program = "add(a,1). add(b,2)."
              map "add" {
                sql = "SELECT '{1}', {2}"
                order_by = 2
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.scenarios[0].maps[0].order_by, Some(2));
    }

    #[test]
    fn parse_scenario_check_block() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            scenario "with_checks" {
              program = "a(1)."
              map "a" {
                sql = "SELECT {1}"
              }
              check "positive" {
                assert = ["SELECT 1 > 0"]
              }
              check "not_null" {
                assert = ["SELECT 1 IS NOT NULL", "SELECT 2 IS NOT NULL"]
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.scenarios[0].checks.len(), 2);
        assert_eq!(cfg.scenarios[0].checks[0].name, "positive");
        assert_eq!(cfg.scenarios[0].checks[0].asserts.len(), 1);
        assert_eq!(cfg.scenarios[0].checks[1].name, "not_null");
        assert_eq!(cfg.scenarios[0].checks[1].asserts.len(), 2);
    }

    #[test]
    fn parse_scenario_expect_error() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            scenario "errors_expected" {
              program = "bad(1)."
              expect_error = true
              map "bad" {
                sql = "SELECT {1}"
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.scenarios[0].expect_error, true);
    }

    #[test]
    fn parse_scenario_assert_eq_and_snapshot() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            scenario "with_assertions" {
              program = "a(1)."
              map "a" {
                sql = "SELECT {1}"
              }
              assert_eq {
                query    = "SELECT 'hello'"
                expected = "hello"
              }
              assert_snapshot {
                query = "SELECT 1 AS a, 2 AS b"
                rows = [
                  ["1", "2"],
                ]
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.scenarios[0].assert_eq.len(), 1);
        assert_eq!(cfg.scenarios[0].assert_eq[0].query, "SELECT 'hello'");
        assert_eq!(cfg.scenarios[0].assert_eq[0].expected, "hello");
        assert_eq!(cfg.scenarios[0].assert_snapshot.len(), 1);
        assert_eq!(cfg.scenarios[0].assert_snapshot[0].rows, vec![vec!["1".to_string(), "2".to_string()]]);
    }

    #[test]
    fn parse_scenario_params() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            scenario "parameterized" {
              program = "a(1..max_items)."
              params = {
                max_items = "3"
                threshold = "100"
              }
              map "a" {
                sql = "SELECT {1}"
              }
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.scenarios[0].params.len(), 2);
        let params: std::collections::HashMap<String, String> = cfg.scenarios[0].params.iter().cloned().collect();
        assert_eq!(params.get("max_items").unwrap(), "3");
        assert_eq!(params.get("threshold").unwrap(), "100");
    }

    #[test]
    fn parse_scenario_teardown() {
        let mut files = HashMap::new();
        files.insert(
            p("/root/main.hcl"),
            r#"
            scenario "with_teardown" {
              program = "a(1)."
              map "a" {
                sql = "SELECT {1}"
              }
              teardown = [
                "DELETE FROM test_table",
                "DROP TABLE IF EXISTS test_table",
              ]
            }
            "#
            .to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.scenarios[0].teardown.len(), 2);
        assert_eq!(cfg.scenarios[0].teardown[0], "DELETE FROM test_table");
        assert_eq!(cfg.scenarios[0].teardown[1], "DROP TABLE IF EXISTS test_table");
    }
}
