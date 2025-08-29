use anyhow::{bail, Context, Result};
use hcl::Body;

use crate::model::*;
use crate::eval::for_each::ForEachSupport;
use crate::eval::core::{expr_to_string_vec, find_attr, get_attr_string, get_attr_bool};

// Schema implementation
impl ForEachSupport for crate::model::SchemaSpec {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let if_not_exists = get_attr_bool(body, "if_not_exists", env)?.unwrap_or(true);
        let authorization = get_attr_string(body, "authorization", env)?;
        Ok(SchemaSpec { name: name.to_string(), alt_name, if_not_exists, authorization })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.schemas.push(item);
    }
}

// Table implementation
impl ForEachSupport for crate::model::TableSpec {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let if_not_exists = get_attr_bool(body, "if_not_exists", env)?.unwrap_or(true);

        // columns
        let mut columns = Vec::new();
        for cblk in body.blocks().filter(|bb| bb.identifier() == "column") {
            let cname = cblk
                .labels()
                .get(0)
                .ok_or_else(|| anyhow::anyhow!("column block missing name label"))?
                .as_str()
                .to_string();
            let cb = cblk.body();
            let ctype = get_attr_string(cb, "type", env)?
                .with_context(|| format!("column '{}' missing type", cname))?;
                let nullable = get_attr_bool(cb, "nullable", env)?.unwrap_or(true);
                let default = get_attr_string(cb, "default", env)?;
                let db_type = get_attr_string(cb, "db_type", env)?;
                columns.push(ColumnSpec { name: cname, r#type: ctype, nullable, default, db_type });
        }

        // primary_key
        let mut primary_key = None;
        for pkblk in body.blocks().filter(|bb| bb.identifier() == "primary_key") {
            let pb = pkblk.body();
            let cols = match find_attr(pb, "columns") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => bail!("primary_key requires columns = [..]"),
            };
            let name = get_attr_string(pb, "name", env)?;
            primary_key = Some(PrimaryKeySpec { name, columns: cols });
        }

        // indexes
        let mut indexes = Vec::new();
        for iblk in body.blocks().filter(|bb| bb.identifier() == "index") {
            let name_attr = iblk.labels().get(0).map(|s| s.as_str().to_string());
            let ib = iblk.body();
            let cols = match find_attr(ib, "columns") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => bail!("index requires columns = [..]"),
            };
            let unique = get_attr_bool(ib, "unique", env)?.unwrap_or(false);
            indexes.push(IndexSpec { name: name_attr, columns: cols, unique });
        }
        for ublk in body.blocks().filter(|bb| bb.identifier() == "unique") {
            let name_attr = ublk.labels().get(0).map(|s| s.as_str().to_string());
            let ub = ublk.body();
            let cols = match find_attr(ub, "columns") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => bail!("unique requires columns = [..]"),
            };
            indexes.push(IndexSpec { name: name_attr, columns: cols, unique: true });
        }

        // foreign keys
        let mut fks = Vec::new();
        for fkblk in body.blocks().filter(|bb| bb.identifier() == "foreign_key") {
            let fb = fkblk.body();
            let columns = match find_attr(fb, "columns") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => bail!("foreign_key requires columns = [..]"),
            };
            // ref {} block
            let mut ref_schema = None;
            let mut ref_table = None;
            let mut ref_columns = None;
            for rblk in fb.blocks().filter(|bb| bb.identifier() == "ref") {
                let rb = rblk.body();
                ref_schema = get_attr_string(rb, "schema", env)?;
                ref_table = get_attr_string(rb, "table", env)?;
                ref_columns = Some(match find_attr(rb, "columns") {
                    Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                    None => bail!("foreign_key.ref requires columns = [..]"),
                });
            }
            let name = get_attr_string(fb, "name", env)?;
            let on_delete = get_attr_string(fb, "on_delete", env)?;
            let on_update = get_attr_string(fb, "on_update", env)?;
            let ref_table = ref_table.context("foreign_key.ref requires table")?;
            let ref_columns = ref_columns.context("foreign_key.ref requires columns = [..]")?;
            fks.push(ForeignKeySpec { name, columns, ref_schema, ref_table, ref_columns, on_delete, on_update });
        }

        Ok(TableSpec { name: name.to_string(), alt_name, schema, if_not_exists, columns, primary_key, indexes, foreign_keys: fks })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.tables.push(item);
    }
}

// View implementation
impl ForEachSupport for crate::model::ViewSpec {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let replace = get_attr_bool(body, "replace", env)?.unwrap_or(true);
        let sql = get_attr_string(body, "sql", env)?.context("view 'sql' is required")?;
        Ok(ViewSpec { name: name.to_string(), alt_name, schema, replace, sql })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.views.push(item);
    }
}

// MaterializedView implementation
impl ForEachSupport for crate::model::MaterializedViewSpec {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let with_data = get_attr_bool(body, "with_data", env)?.unwrap_or(true);
        let sql = get_attr_string(body, "sql", env)?.context("materialized 'sql' is required")?;
        Ok(MaterializedViewSpec { name: name.to_string(), alt_name, schema, with_data, sql })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.materialized.push(item);
    }
}

// Policy implementation
impl ForEachSupport for crate::model::PolicySpec {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let table = get_attr_string(body, "table", env)?.context("policy 'table' is required")?;
        let command = get_attr_string(body, "command", env)?.unwrap_or_else(|| "ALL".to_string());
        let as_kind = get_attr_string(body, "as", env)?;
        let roles = match find_attr(body, "roles") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let using = get_attr_string(body, "using", env)?;
        let check = get_attr_string(body, "check", env)?;
        Ok(PolicySpec { name: name.to_string(), alt_name, schema, table, command, r#as: as_kind, roles, using, check })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.policies.push(item);
    }
}

// Function implementation
impl ForEachSupport for crate::model::FunctionSpec {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let language = get_attr_string(body, "language", env)?.unwrap_or_else(|| "plpgsql".to_string());
        let body_sql = get_attr_string(body, "body", env)?.context("function 'body' is required")?;
        let returns = get_attr_string(body, "returns", env)?.unwrap_or_else(|| "trigger".to_string());
        let schema = get_attr_string(body, "schema", env)?;
        let replace = get_attr_bool(body, "replace", env)?.unwrap_or(true);
        let security_definer = get_attr_bool(body, "security_definer", env)?.unwrap_or(false);
        Ok(FunctionSpec {
            name: name.to_string(),
            alt_name,
            schema,
            language,
            returns,
            replace,
            security_definer,
            body: body_sql,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.functions.push(item);
    }
}

// Trigger implementation
impl ForEachSupport for crate::model::TriggerSpec {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let table = get_attr_string(body, "table", env)?.context("trigger 'table' is required")?;
        let timing = get_attr_string(body, "timing", env)?.unwrap_or_else(|| "BEFORE".to_string());
        let events = match find_attr(body, "events") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => vec!["UPDATE".to_string()],
        };
        let level = get_attr_string(body, "level", env)?.unwrap_or_else(|| "ROW".to_string());
        let function = get_attr_string(body, "function", env)?.context("trigger 'function' is required")?;
        let function_schema = get_attr_string(body, "function_schema", env)?;
        let when = get_attr_string(body, "when", env)?;
        Ok(TriggerSpec {
            name: name.to_string(),
            alt_name,
            schema,
            table,
            timing,
            events,
            level,
            function,
            function_schema,
            when,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.triggers.push(item);
    }
}

// Extension implementation
impl ForEachSupport for crate::model::ExtensionSpec {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let if_not_exists = get_attr_bool(body, "if_not_exists", env)?.unwrap_or(true);
        let schema = get_attr_string(body, "schema", env)?;
        let version = get_attr_string(body, "version", env)?;
        Ok(ExtensionSpec { name: name.to_string(), alt_name, if_not_exists, schema, version })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.extensions.push(item);
    }
}

// Enum implementation
impl ForEachSupport for crate::model::EnumSpec {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let values = match find_attr(body, "values") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => bail!("enum '{}' requires values = [..]", name),
        };
        Ok(EnumSpec { name: name.to_string(), alt_name, schema, values })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.enums.push(item);
    }
}