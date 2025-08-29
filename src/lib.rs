pub mod parser;
pub mod eval;
pub mod model;
pub mod config;
pub mod backends;
pub mod test_runner;

use anyhow::{bail, Result};
// Keep types public via re-exports
use std::path::Path;

// Public re-exports
pub use model::{Config, EnvVars, ExtensionSpec, FunctionSpec, TriggerSpec, TableSpec, ViewSpec, MaterializedViewSpec, EnumSpec, SchemaSpec, PolicySpec};

/// Resource kinds that can be filtered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Schemas,
    Enums,
    Tables,
    Views,
    Materialized,
    Functions,
    Triggers,
    Extensions,
    Policies,
    Tests,
}

// Loader abstraction: lets callers control how files are read.
pub trait Loader {
    fn load(&self, path: &Path) -> Result<String>;
}

// Pure API: parse + evaluate HCL config starting at `root_path` using a Loader.
pub fn load_config(root_path: &Path, loader: &dyn Loader, env: EnvVars) -> Result<Config> {
    parser::load_root_with_loader(root_path, loader, env)
}

// Pure validation: check references etc.
pub fn validate(cfg: &Config) -> Result<()> {
    for t in &cfg.triggers {
        let fqn = format!(
            "{}.{}",
            t.function_schema.as_deref().unwrap_or("public"),
            t.function
        );
        let found = cfg.functions.iter().any(|f| {
            let fs = f.schema.as_deref().unwrap_or("public");
            f.name == t.function && (t.function_schema.as_deref().unwrap_or(fs) == fs)
        });
        if !found {
            bail!(
                "trigger '{}' references missing function '{}': ensure function exists or set function_schema",
                t.name, fqn
            );
        }
    }
    Ok(())
}

/// Apply filters to a configuration based on target settings
pub fn apply_filters(
    cfg: &Config,
    include: &std::collections::HashSet<crate::config::ResourceKind>,
    exclude: &std::collections::HashSet<crate::config::ResourceKind>,
) -> Config {
    use crate::config::ResourceKind as R;

    Config {
        functions: if include.contains(&R::Functions) && !exclude.contains(&R::Functions) {
            cfg.functions.clone()
        } else {
            Vec::new()
        },
        triggers: if include.contains(&R::Triggers) && !exclude.contains(&R::Triggers) {
            cfg.triggers.clone()
        } else {
            Vec::new()
        },
        extensions: if include.contains(&R::Extensions) && !exclude.contains(&R::Extensions) {
            cfg.extensions.clone()
        } else {
            Vec::new()
        },
        schemas: if include.contains(&R::Schemas) && !exclude.contains(&R::Schemas) {
            cfg.schemas.clone()
        } else {
            Vec::new()
        },
        enums: if include.contains(&R::Enums) && !exclude.contains(&R::Enums) {
            cfg.enums.clone()
        } else {
            Vec::new()
        },
        tables: if include.contains(&R::Tables) && !exclude.contains(&R::Tables) {
            cfg.tables.clone()
        } else {
            Vec::new()
        },
        views: if include.contains(&R::Views) && !exclude.contains(&R::Views) {
            cfg.views.clone()
        } else {
            Vec::new()
        },
        materialized: if include.contains(&R::Materialized) && !exclude.contains(&R::Materialized) {
            cfg.materialized.clone()
        } else {
            Vec::new()
        },
        policies: if include.contains(&R::Policies) && !exclude.contains(&R::Policies) {
            cfg.policies.clone()
        } else {
            Vec::new()
        },
        tests: if include.contains(&R::Tests) && !exclude.contains(&R::Tests) {
            cfg.tests.clone()
        } else {
            Vec::new()
        },
    }
}

/// Apply resource filters to a configuration (string-based for TOML config)
pub fn apply_resource_filters(cfg: &Config, include: &[String], exclude: &[String]) -> Config {
    use model::*;

    // If no filters specified, include everything
    let include_all = include.is_empty() && exclude.is_empty();

    // Convert filter lists to sets for efficient lookup
    let include_set: std::collections::HashSet<String> = include.iter().cloned().collect();
    let exclude_set: std::collections::HashSet<String> = exclude.iter().cloned().collect();

    // Helper function to check if a resource type should be included
    let should_include = |resource_type: &str| -> bool {
        if include_all {
            true
        } else if !include_set.is_empty() {
            include_set.contains(resource_type)
        } else {
            !exclude_set.contains(resource_type)
        }
    };

    Config {
        functions: if should_include("functions") { cfg.functions.clone() } else { Vec::new() },
        triggers: if should_include("triggers") { cfg.triggers.clone() } else { Vec::new() },
        extensions: if should_include("extensions") { cfg.extensions.clone() } else { Vec::new() },
        schemas: if should_include("schemas") { cfg.schemas.clone() } else { Vec::new() },
        enums: if should_include("enums") { cfg.enums.clone() } else { Vec::new() },
        tables: if should_include("tables") { cfg.tables.clone() } else { Vec::new() },
        views: if should_include("views") { cfg.views.clone() } else { Vec::new() },
        materialized: if should_include("materialized") { cfg.materialized.clone() } else { Vec::new() },
        policies: if should_include("policies") { cfg.policies.clone() } else { Vec::new() },
        tests: if should_include("tests") { cfg.tests.clone() } else { Vec::new() },
    }
}

// Pure SQL generation wrapper - uses postgres backend by default
pub fn generate_sql(cfg: &Config) -> Result<String> {
    backends::postgres::to_sql(cfg)
}

pub fn generate_with_backend(backend: &str, cfg: &Config, env: &EnvVars) -> Result<String> {
    let be = backends::get_backend(backend)
        .ok_or_else(|| anyhow::anyhow!(format!("unknown backend '{backend}'")))?;
    be.generate(cfg, env)
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

    fn p(s: &str) -> PathBuf { PathBuf::from(s) }

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
        validate(&cfg).unwrap();
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
        validate(&cfg).unwrap();
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
        validate(&cfg).unwrap();
        let sql = generate_sql(&cfg).unwrap();
        assert!(sql.contains("\"users\""));
        assert!(sql.contains("\"orders\""));
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
        let json = crate::generate_with_backend("json", &cfg, &env).unwrap();
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
            "#.to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.views.len(), 1);
        let sql = generate_sql(&cfg).unwrap();
        assert!(sql.contains("CREATE OR REPLACE VIEW \"public\".\"v_users\" AS"));
        let env = EnvVars::default();
        let json = crate::generate_with_backend("json", &cfg, &env).unwrap();
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
            "#.to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.materialized.len(), 1);
        let sql = generate_sql(&cfg).unwrap();
        assert!(sql.contains("CREATE MATERIALIZED VIEW \"public\".\"mv\" AS"));
        assert!(sql.contains("WITH NO DATA"));
        let env = EnvVars::default();
        let json = crate::generate_with_backend("json", &cfg, &env).unwrap();
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
            "#.to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.enums.len(), 1);
        let sql = generate_sql(&cfg).unwrap();
        assert!(sql.contains("CREATE TYPE \"public\".\"status\" AS ENUM"));
        let env = EnvVars::default();
        let json = crate::generate_with_backend("json", &cfg, &env).unwrap();
        assert!(json.contains("\"enums\""));
        let env = EnvVars::default();
        let prisma = crate::generate_with_backend("prisma", &cfg, &env).unwrap();
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
            "#.to_string(),
        );
        let loader = MapLoader { files };
        let cfg = load_config(&p("/root/main.hcl"), &loader, EnvVars::default()).unwrap();
        assert_eq!(cfg.policies.len(), 1);
        let sql = generate_sql(&cfg).unwrap();
        assert!(sql.contains("CREATE POLICY \"p_users_select\" ON \"public\".\"users\""));
        let env = EnvVars::default();
        let json = crate::generate_with_backend("json", &cfg, &env).unwrap();
        assert!(json.contains("\"policies\""));
        assert!(json.contains("\"p_users_select\""));
    }
}
