use anyhow::{Context, Result, bail};
use hcl::{Body, Value};

use crate::frontend::ast::*;
use crate::frontend::core::{
    expr_to_string_vec, expr_to_value, find_attr, get_attr_bool, get_attr_string,
};
use crate::frontend::env::EnvVars;
use crate::frontend::for_each::ForEachSupport;

// Schema implementation
impl ForEachSupport for AstSchema {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let if_not_exists = get_attr_bool(body, "if_not_exists", env)?.unwrap_or(true);
        let authorization = get_attr_string(body, "authorization", env)?;
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstSchema {
            name: name.to_string(),
            alt_name,
            if_not_exists,
            authorization,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.schemas.push(item);
    }
}

// Sequence implementation
impl ForEachSupport for AstSequence {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let if_not_exists = get_attr_bool(body, "if_not_exists", env)?.unwrap_or(true);
        let r#as = get_attr_string(body, "as", env)?;
        let comment = get_attr_string(body, "comment", env)?;
        let parse_i64 = |attr: &str| -> Result<Option<i64>> {
            match get_attr_string(body, attr, env)? {
                Some(s) => Ok(Some(
                    s.parse::<i64>()
                        .with_context(|| format!("{} must be an integer", attr))?,
                )),
                None => Ok(None),
            }
        };
        let increment = parse_i64("increment")?;
        let min_value = parse_i64("min_value")?;
        let max_value = parse_i64("max_value")?;
        let start = parse_i64("start")?;
        let cache = parse_i64("cache")?;
        let cycle = get_attr_bool(body, "cycle", env)?.unwrap_or(false);
        let owned_by = get_attr_string(body, "owned_by", env)?;
        Ok(AstSequence {
            name: name.to_string(),
            alt_name,
            schema,
            if_not_exists,
            r#as,
            increment,
            min_value,
            max_value,
            start,
            cache,
            cycle,
            owned_by,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.sequences.push(item);
    }
}

// Table implementation
impl ForEachSupport for AstTable {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "table_name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let if_not_exists = get_attr_bool(body, "if_not_exists", env)?.unwrap_or(true);
        let comment = get_attr_string(body, "comment", env)?;
        let map = get_attr_string(body, "map", env)?;

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
            let comment = get_attr_string(cb, "comment", env)?;
            let lint_ignore = match find_attr(cb, "lint_ignore") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => Vec::new(),
            };
            let count = match get_attr_string(cb, "count", env)? {
                Some(s) => s.parse::<usize>().unwrap_or(1),
                None => 1,
            };
            if count > 0 {
                columns.push(AstColumn {
                    name: cname,
                    r#type: ctype,
                    nullable,
                    default,
                    db_type,
                    lint_ignore,
                    comment,
                    count,
                });
            }
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
            primary_key = Some(AstPrimaryKey {
                name,
                columns: cols,
            });
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
            let exprs = match find_attr(ib, "expressions") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => Vec::new(),
            };
            let where_clause = get_attr_string(ib, "where", env)?;
            let orders = match find_attr(ib, "orders") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => Vec::new(),
            };
            let operator_classes = match find_attr(ib, "operator_classes") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => Vec::new(),
            };
            let unique = get_attr_bool(ib, "unique", env)?.unwrap_or(false);
            indexes.push(AstIndex {
                name: name_attr,
                columns: cols,
                expressions: exprs,
                r#where: where_clause,
                orders,
                operator_classes,
                unique,
            });
        }
        for ublk in body.blocks().filter(|bb| bb.identifier() == "unique") {
            let name_attr = ublk.labels().get(0).map(|s| s.as_str().to_string());
            let ub = ublk.body();
            let cols = match find_attr(ub, "columns") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => bail!("unique requires columns = [..]"),
            };
            let exprs = match find_attr(ub, "expressions") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => Vec::new(),
            };
            let where_clause = get_attr_string(ub, "where", env)?;
            let orders = match find_attr(ub, "orders") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => Vec::new(),
            };
            let operator_classes = match find_attr(ub, "operator_classes") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => Vec::new(),
            };
            indexes.push(AstIndex {
                name: name_attr,
                columns: cols,
                expressions: exprs,
                r#where: where_clause,
                orders,
                operator_classes,
                unique: true,
            });
        }

        // checks
        let mut checks = Vec::new();
        for cblk in body.blocks().filter(|bb| bb.identifier() == "check") {
            let name_attr = cblk.labels().get(0).map(|s| s.as_str().to_string());
            let cb = cblk.body();
            let expression =
                get_attr_string(cb, "expression", env)?.context("check requires expression")?;
            checks.push(AstCheck {
                name: name_attr,
                expression,
            });
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
            let back_reference_name = get_attr_string(fb, "back_reference_name", env)?;
            let ref_table = ref_table.context("foreign_key.ref requires table")?;
            let ref_columns = ref_columns.context("foreign_key.ref requires columns = [..]")?;
            fks.push(AstForeignKey {
                name,
                columns,
                ref_schema,
                ref_table,
                ref_columns,
                on_delete,
                on_update,
                back_reference_name,
            });
        }

        // partitioning
        let mut partition_by = None;
        for pblk in body.blocks().filter(|bb| bb.identifier() == "partition_by") {
            let pb = pblk.body();
            let strategy =
                get_attr_string(pb, "strategy", env)?.context("partition_by requires strategy")?;
            let columns = match find_attr(pb, "columns") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => bail!("partition_by requires columns = [..]"),
            };
            partition_by = Some(AstPartitionBy { strategy, columns });
        }

        let mut partitions = Vec::new();
        for pblk in body.blocks().filter(|bb| bb.identifier() == "partition") {
            let name = pblk
                .labels()
                .get(0)
                .map(|s| s.as_str().to_string())
                .context("partition requires a name")?;
            let pb = pblk.body();
            let values =
                get_attr_string(pb, "values", env)?.context("partition requires values")?;
            partitions.push(AstPartition { name, values });
        }

        let lint_ignore = match find_attr(body, "lint_ignore") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };

        Ok(AstTable {
            name: name.to_string(),
            alt_name,
            schema,
            if_not_exists,
            columns,
            primary_key,
            indexes,
            checks,
            foreign_keys: fks,
            partition_by,
            partitions,
            back_references: Vec::new(),
            lint_ignore,
            comment,
            map,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.tables.push(item);
    }
}

// View implementation
impl ForEachSupport for AstView {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let replace = get_attr_bool(body, "replace", env)?.unwrap_or(true);
        let sql = get_attr_string(body, "sql", env)?.context("view 'sql' is required")?;
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstView {
            name: name.to_string(),
            alt_name,
            schema,
            replace,
            sql,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.views.push(item);
    }
}

// MaterializedView implementation
impl ForEachSupport for AstMaterializedView {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let with_data = get_attr_bool(body, "with_data", env)?.unwrap_or(true);
        let sql = get_attr_string(body, "sql", env)?.context("materialized 'sql' is required")?;
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstMaterializedView {
            name: name.to_string(),
            alt_name,
            schema,
            with_data,
            sql,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.materialized.push(item);
    }
}

// Policy implementation
impl ForEachSupport for AstPolicy {
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
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstPolicy {
            name: name.to_string(),
            alt_name,
            schema,
            table,
            command,
            r#as: as_kind,
            roles,
            using,
            check,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.policies.push(item);
    }
}

// Function implementation
impl ForEachSupport for AstFunction {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let language =
            get_attr_string(body, "language", env)?.unwrap_or_else(|| "plpgsql".to_string());
        let body_sql =
            get_attr_string(body, "body", env)?.context("function 'body' is required")?;
        let returns =
            get_attr_string(body, "returns", env)?.unwrap_or_else(|| "trigger".to_string());
        let schema = get_attr_string(body, "schema", env)?;
        let parameters = match find_attr(body, "parameters") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let replace = get_attr_bool(body, "replace", env)?.unwrap_or(true);
        let volatility = get_attr_string(body, "volatility", env)?;
        let strict = get_attr_bool(body, "strict", env)?.unwrap_or(false);
        let security = get_attr_string(body, "security", env)?;
        let cost = match get_attr_string(body, "cost", env)? {
            Some(s) => Some(
                s.parse::<f64>()
                    .context("function 'cost' must be a number")?,
            ),
            None => None,
        };
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstFunction {
            name: name.to_string(),
            alt_name,
            schema,
            language,
            parameters,
            returns,
            replace,
            volatility,
            strict,
            security,
            cost,
            body: body_sql,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.functions.push(item);
    }
}

// Procedure implementation
impl ForEachSupport for AstProcedure {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let language =
            get_attr_string(body, "language", env)?.unwrap_or_else(|| "plpgsql".to_string());
        let body_sql =
            get_attr_string(body, "body", env)?.context("procedure 'body' is required")?;
        let schema = get_attr_string(body, "schema", env)?;
        let parameters = match find_attr(body, "parameters") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let replace = get_attr_bool(body, "replace", env)?.unwrap_or(true);
        let security = get_attr_string(body, "security", env)?;
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstProcedure {
            name: name.to_string(),
            alt_name,
            schema,
            language,
            parameters,
            replace,
            security,
            body: body_sql,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.procedures.push(item);
    }
}

// Aggregate implementation
impl ForEachSupport for AstAggregate {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let inputs = match find_attr(body, "inputs") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let sfunc =
            get_attr_string(body, "sfunc", env)?.context("aggregate 'sfunc' is required")?;
        let stype =
            get_attr_string(body, "stype", env)?.context("aggregate 'stype' is required")?;
        let finalfunc = get_attr_string(body, "finalfunc", env)?;
        let initcond = get_attr_string(body, "initcond", env)?;
        let parallel = get_attr_string(body, "parallel", env)?;
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstAggregate {
            name: name.to_string(),
            alt_name,
            schema,
            inputs,
            sfunc,
            stype,
            finalfunc,
            initcond,
            parallel,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.aggregates.push(item);
    }
}

// Operator implementation
impl ForEachSupport for AstOperator {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let left = get_attr_string(body, "left", env)?;
        let right = get_attr_string(body, "right", env)?;
        let procedure =
            get_attr_string(body, "procedure", env)?.context("operator 'procedure' is required")?;
        let commutator = get_attr_string(body, "commutator", env)?;
        let negator = get_attr_string(body, "negator", env)?;
        let restrict = get_attr_string(body, "restrict", env)?;
        let join = get_attr_string(body, "join", env)?;
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstOperator {
            name: name.to_string(),
            alt_name,
            schema,
            left,
            right,
            procedure,
            commutator,
            negator,
            restrict,
            join,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.operators.push(item);
    }
}

// Trigger implementation
impl ForEachSupport for AstTrigger {
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
        let function =
            get_attr_string(body, "function", env)?.context("trigger 'function' is required")?;
        let function_schema = get_attr_string(body, "function_schema", env)?;
        let when = get_attr_string(body, "when", env)?;
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstTrigger {
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
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.triggers.push(item);
    }
}

// Rule implementation
impl ForEachSupport for AstRule {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let table = get_attr_string(body, "table", env)?.context("rule 'table' is required")?;
        let event = get_attr_string(body, "event", env)?.context("rule 'event' is required")?;
        let r#where = get_attr_string(body, "where", env)?;
        let instead = get_attr_bool(body, "instead", env)?.unwrap_or(false);
        let command =
            get_attr_string(body, "command", env)?.context("rule 'command' is required")?;
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstRule {
            name: name.to_string(),
            alt_name,
            schema,
            table,
            event,
            r#where,
            instead,
            command,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.rules.push(item);
    }
}

// EventTrigger implementation
impl ForEachSupport for AstEventTrigger {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let event =
            get_attr_string(body, "event", env)?.context("event_trigger 'event' is required")?;
        let tags = match find_attr(body, "tags") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let function = get_attr_string(body, "function", env)?
            .context("event_trigger 'function' is required")?;
        let function_schema = get_attr_string(body, "function_schema", env)?;
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstEventTrigger {
            name: name.to_string(),
            alt_name,
            event,
            tags,
            function,
            function_schema,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.event_triggers.push(item);
    }
}

// Extension implementation
impl ForEachSupport for AstExtension {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let if_not_exists = get_attr_bool(body, "if_not_exists", env)?.unwrap_or(true);
        let schema = get_attr_string(body, "schema", env)?;
        let version = get_attr_string(body, "version", env)?;
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstExtension {
            name: name.to_string(),
            alt_name,
            if_not_exists,
            schema,
            version,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.extensions.push(item);
    }
}

// Collation implementation
impl ForEachSupport for AstCollation {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let if_not_exists = get_attr_bool(body, "if_not_exists", env)?.unwrap_or(true);
        let from = get_attr_string(body, "from", env)?;
        let locale = get_attr_string(body, "locale", env)?;
        let lc_collate = get_attr_string(body, "lc_collate", env)?;
        let lc_ctype = get_attr_string(body, "lc_ctype", env)?;
        let provider = get_attr_string(body, "provider", env)?;
        let deterministic = get_attr_bool(body, "deterministic", env)?;
        let version = get_attr_string(body, "version", env)?;
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstCollation {
            name: name.to_string(),
            alt_name,
            schema,
            if_not_exists,
            from,
            locale,
            lc_collate,
            lc_ctype,
            provider,
            deterministic,
            version,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.collations.push(item);
    }
}

// Enum implementation
impl ForEachSupport for AstEnum {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let values = match find_attr(body, "values") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => bail!("enum '{}' requires values = [..]", name),
        };
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstEnum {
            name: name.to_string(),
            alt_name,
            schema,
            values,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.enums.push(item);
    }
}

// Domain implementation
impl ForEachSupport for AstDomain {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let r#type = get_attr_string(body, "type", env)?
            .with_context(|| format!("domain '{}' missing type", name))?;
        let not_null = get_attr_bool(body, "not_null", env)?.unwrap_or(false);
        let default = get_attr_string(body, "default", env)?;
        let constraint = get_attr_string(body, "constraint", env)?;
        let check = get_attr_string(body, "check", env)?;
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstDomain {
            name: name.to_string(),
            alt_name,
            schema,
            r#type,
            not_null,
            default,
            constraint,
            check,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.domains.push(item);
    }
}

// Composite type implementation
impl ForEachSupport for AstCompositeType {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let comment = get_attr_string(body, "comment", env)?;
        let mut fields = Vec::new();
        for fblk in body.blocks().filter(|bb| bb.identifier() == "field") {
            let fname = fblk
                .labels()
                .get(0)
                .ok_or_else(|| anyhow::anyhow!("field block missing name label"))?
                .as_str()
                .to_string();
            let fb = fblk.body();
            let ftype = get_attr_string(fb, "type", env)?
                .with_context(|| format!("field '{}' missing type", fname))?;
            fields.push(AstCompositeTypeField {
                name: fname,
                r#type: ftype,
            });
        }
        Ok(AstCompositeType {
            name: name.to_string(),
            alt_name,
            schema,
            fields,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.types.push(item);
    }
}

// Role implementation
impl ForEachSupport for AstRole {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let login = get_attr_bool(body, "login", env)?.unwrap_or(false);
        let superuser = get_attr_bool(body, "superuser", env)?.unwrap_or(false);
        let createdb = get_attr_bool(body, "createdb", env)?.unwrap_or(false);
        let createrole = get_attr_bool(body, "createrole", env)?.unwrap_or(false);
        let replication = get_attr_bool(body, "replication", env)?.unwrap_or(false);
        let password = get_attr_string(body, "password", env)?;
        let in_role = match find_attr(body, "in_role") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstRole {
            name: name.to_string(),
            alt_name,
            login,
            superuser,
            createdb,
            createrole,
            replication,
            password,
            in_role,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.roles.push(item);
    }
}

// Tablespace implementation
impl ForEachSupport for AstTablespace {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let location =
            get_attr_string(body, "location", env)?.context("tablespace 'location' is required")?;
        let owner = get_attr_string(body, "owner", env)?;
        let options = match find_attr(body, "options") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstTablespace {
            name: name.to_string(),
            alt_name,
            location,
            owner,
            options,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.tablespaces.push(item);
    }
}

// Grant implementation
impl ForEachSupport for AstGrant {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let role = get_attr_string(body, "role", env)?.context("grant 'role' is required")?;
        let privileges = match find_attr(body, "privileges") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => bail!("grant requires privileges = [..]"),
        };
        let schema = get_attr_string(body, "schema", env)?;
        let table = get_attr_string(body, "table", env)?;
        let function = get_attr_string(body, "function", env)?;
        let database = get_attr_string(body, "database", env)?;
        let sequence = get_attr_string(body, "sequence", env)?;
        if table.is_none()
            && function.is_none()
            && schema.is_none()
            && database.is_none()
            && sequence.is_none()
        {
            bail!("grant requires table, schema, function, database, or sequence");
        }
        Ok(AstGrant {
            name: name.to_string(),
            alt_name,
            role,
            privileges,
            schema,
            table,
            function,
            database,
            sequence,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.grants.push(item);
    }
}

// Index implementation
impl ForEachSupport for AstStandaloneIndex {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let table = get_attr_string(body, "table", env)?.context("index 'table' is required")?;
        let schema = get_attr_string(body, "schema", env)?;
        let cols = match find_attr(body, "columns") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => bail!("index requires columns = [..]"),
        };
        let exprs = match find_attr(body, "expressions") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let where_clause = get_attr_string(body, "where", env)?;
        let orders = match find_attr(body, "orders") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let operator_classes = match find_attr(body, "operator_classes") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let unique = get_attr_bool(body, "unique", env)?.unwrap_or(false);
        Ok(AstStandaloneIndex {
            name: name.to_string(),
            table,
            schema,
            columns: cols,
            expressions: exprs,
            r#where: where_clause,
            orders,
            operator_classes,
            unique,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.indexes.push(item);
    }
}

// Statistics implementation
impl ForEachSupport for AstStatistics {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let table =
            get_attr_string(body, "table", env)?.context("statistics 'table' is required")?;
        let columns = match find_attr(body, "columns") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => bail!("statistics requires columns = [..]"),
        };
        let kinds = match find_attr(body, "kinds") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstStatistics {
            name: name.to_string(),
            alt_name,
            schema,
            table,
            columns,
            kinds,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.statistics.push(item);
    }
}

// Foreign Data Wrapper implementation
impl ForEachSupport for AstForeignDataWrapper {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let handler = get_attr_string(body, "handler", env)?;
        let validator = get_attr_string(body, "validator", env)?;
        let options = match find_attr(body, "options") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstForeignDataWrapper {
            name: name.to_string(),
            alt_name,
            handler,
            validator,
            options,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.foreign_data_wrappers.push(item);
    }
}

// Foreign Server implementation
impl ForEachSupport for AstForeignServer {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let wrapper = get_attr_string(body, "wrapper", env)?
            .context("foreign_server 'wrapper' is required")?;
        let r#type = get_attr_string(body, "type", env)?;
        let version = get_attr_string(body, "version", env)?;
        let options = match find_attr(body, "options") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstForeignServer {
            name: name.to_string(),
            alt_name,
            wrapper,
            r#type,
            version,
            options,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.foreign_servers.push(item);
    }
}

// Foreign Table implementation
impl ForEachSupport for AstForeignTable {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let server =
            get_attr_string(body, "server", env)?.context("foreign_table 'server' is required")?;
        let comment = get_attr_string(body, "comment", env)?;

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
            let comment = get_attr_string(cb, "comment", env)?;
            let lint_ignore = match find_attr(cb, "lint_ignore") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => Vec::new(),
            };
            let count = match get_attr_string(cb, "count", env)? {
                Some(s) => s.parse::<usize>().unwrap_or(1),
                None => 1,
            };
            if count > 0 {
                columns.push(AstColumn {
                    name: cname,
                    r#type: ctype,
                    nullable,
                    default,
                    db_type,
                    lint_ignore,
                    comment,
                    count,
                });
            }
        }

        let options = match find_attr(body, "options") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };

        Ok(AstForeignTable {
            name: name.to_string(),
            alt_name,
            schema,
            server,
            columns,
            options,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.foreign_tables.push(item);
    }
}

// Text Search Dictionary implementation
impl ForEachSupport for AstTextSearchDictionary {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let template = get_attr_string(body, "template", env)?
            .context("text_search_dictionary 'template' is required")?;
        let options = match find_attr(body, "options") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstTextSearchDictionary {
            name: name.to_string(),
            alt_name,
            schema,
            template,
            options,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.text_search_dictionaries.push(item);
    }
}

// Text Search Configuration implementation
impl ForEachSupport for AstTextSearchConfiguration {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let parser = get_attr_string(body, "parser", env)?
            .context("text_search_configuration 'parser' is required")?;
        let comment = get_attr_string(body, "comment", env)?;

        let mut mappings = Vec::new();
        for mblk in body.blocks().filter(|bb| bb.identifier() == "mapping") {
            let mb = mblk.body();
            let tokens = match find_attr(mb, "for") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => bail!("mapping missing 'for' attribute"),
            };
            let dictionaries = match find_attr(mb, "with") {
                Some(attr) => expr_to_string_vec(attr.expr(), env)?,
                None => bail!("mapping missing 'with' attribute"),
            };
            mappings.push(AstTextSearchConfigurationMapping {
                tokens,
                dictionaries,
            });
        }

        Ok(AstTextSearchConfiguration {
            name: name.to_string(),
            alt_name,
            schema,
            parser,
            mappings,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.text_search_configurations.push(item);
    }
}

// Text Search Template implementation
impl ForEachSupport for AstTextSearchTemplate {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let lexize = get_attr_string(body, "lexize", env)?
            .context("text_search_template 'lexize' is required")?;
        let init = get_attr_string(body, "init", env)?;
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstTextSearchTemplate {
            name: name.to_string(),
            alt_name,
            schema,
            init,
            lexize,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.text_search_templates.push(item);
    }
}

// Text Search Parser implementation
impl ForEachSupport for AstTextSearchParser {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let schema = get_attr_string(body, "schema", env)?;
        let start = get_attr_string(body, "start", env)?
            .context("text_search_parser 'start' is required")?;
        let gettoken = get_attr_string(body, "gettoken", env)?
            .context("text_search_parser 'gettoken' is required")?;
        let end =
            get_attr_string(body, "end", env)?.context("text_search_parser 'end' is required")?;
        let headline = get_attr_string(body, "headline", env)?;
        let lextypes = get_attr_string(body, "lextypes", env)?
            .context("text_search_parser 'lextypes' is required")?;
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstTextSearchParser {
            name: name.to_string(),
            alt_name,
            schema,
            start,
            gettoken,
            end,
            headline,
            lextypes,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.text_search_parsers.push(item);
    }
}

// Publication implementation
impl ForEachSupport for AstPublication {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let all_tables = get_attr_bool(body, "all_tables", env)?.unwrap_or(false);
        let tables = match find_attr(body, "tables") {
            Some(attr) => {
                let val = expr_to_value(attr.expr(), env)?;
                match val {
                    Value::Array(arr) => {
                        let mut out = Vec::new();
                        for v in arr {
                            match v {
                                Value::Object(mut obj) => {
                                    let table = match obj.swap_remove("table") {
                                        Some(Value::String(s)) => s,
                                        _ => bail!("tables[].table is required"),
                                    };
                                    let schema = match obj.swap_remove("schema") {
                                        Some(Value::String(s)) => Some(s),
                                        None => None,
                                        Some(other) => {
                                            bail!("tables[].schema must be string, got {other:?}")
                                        }
                                    };
                                    out.push(AstPublicationTable { schema, table });
                                }
                                _ => bail!("tables must be array of objects"),
                            }
                        }
                        out
                    }
                    _ => bail!("tables must be array of objects"),
                }
            }
            None => Vec::new(),
        };
        let publish = match find_attr(body, "publish") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstPublication {
            name: name.to_string(),
            alt_name,
            all_tables,
            tables,
            publish,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.publications.push(item);
    }
}

// Subscription implementation
impl ForEachSupport for AstSubscription {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let alt_name = get_attr_string(body, "name", env)?;
        let connection = get_attr_string(body, "connection", env)?
            .context("subscription 'connection' is required")?;
        let publications = match find_attr(body, "publications") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => bail!("subscription requires publications = [..]"),
        };
        let comment = get_attr_string(body, "comment", env)?;
        Ok(AstSubscription {
            name: name.to_string(),
            alt_name,
            connection,
            publications,
            comment,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.subscriptions.push(item);
    }
}

// Test implementation
impl ForEachSupport for AstTest {
    type Item = Self;

    fn parse_one(name: &str, body: &Body, env: &EnvVars) -> Result<Self::Item> {
        let test_name = if let Some((key, _)) = &env.each {
            format!("{}[{}]", name, crate::frontend::core::value_to_string(key)?)
        } else if let Some(idx) = env.count {
            format!("{}[{}]", name, idx)
        } else {
            name.to_string()
        };
        let setup = match find_attr(body, "setup") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let asserts: Vec<String> = match find_attr(body, "assert") {
            Some(attr) => match expr_to_string_vec(attr.expr(), env) {
                Ok(v) => v,
                Err(_) => {
                    vec![get_attr_string(body, "assert", env)?
                        .context("test 'assert' is required")?]
                }
            },
            None => Vec::new(),
        };
        let assert_fail = match find_attr(body, "assert_fail") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        let mut assert_notify = Vec::new();
        for nb in body.blocks().filter(|nb| nb.identifier() == "assert_notify") {
            let nb_body = nb.body();
            let channel = get_attr_string(nb_body, "channel", env)?
                .ok_or_else(|| anyhow::anyhow!("assert_notify missing 'channel'"))?;
            let payload_contains = get_attr_string(nb_body, "payload_contains", env)?;
            assert_notify.push(NotifyAssert {
                channel,
                payload_contains,
            });
        }
        let mut assert_eq = Vec::new();
        for eb in body.blocks().filter(|eb| eb.identifier() == "assert_eq") {
            let eb_body = eb.body();
            let query = get_attr_string(eb_body, "query", env)?
                .ok_or_else(|| anyhow::anyhow!("assert_eq missing 'query'"))?;
            let expected = get_attr_string(eb_body, "expected", env)?
                .ok_or_else(|| anyhow::anyhow!("assert_eq missing 'expected'"))?;
            assert_eq.push(EqAssert { query, expected });
        }
        let mut assert_error = Vec::new();
        for erb in body.blocks().filter(|erb| erb.identifier() == "assert_error") {
            let erb_body = erb.body();
            let sql = get_attr_string(erb_body, "sql", env)?
                .ok_or_else(|| anyhow::anyhow!("assert_error missing 'sql'"))?;
            let message_contains = get_attr_string(erb_body, "message_contains", env)?
                .ok_or_else(|| anyhow::anyhow!("assert_error missing 'message_contains'"))?;
            assert_error.push(ErrorAssert {
                sql,
                message_contains,
            });
        }
        let mut assert_snapshot = Vec::new();
        for sb in body.blocks().filter(|sb| sb.identifier() == "assert_snapshot") {
            let sb_body = sb.body();
            let query = get_attr_string(sb_body, "query", env)?
                .ok_or_else(|| anyhow::anyhow!("assert_snapshot missing 'query'"))?;
            let rows_attr = find_attr(sb_body, "rows")
                .ok_or_else(|| anyhow::anyhow!("assert_snapshot missing 'rows'"))?;
            let rows_val = expr_to_value(rows_attr.expr(), env)?;
            let rows = match rows_val {
                Value::Array(outer) => {
                    let mut result = Vec::new();
                    for row in outer {
                        match row {
                            Value::Array(inner) => {
                                let cols: Vec<String> = inner
                                    .into_iter()
                                    .map(|v| match v {
                                        Value::String(s) => Ok(s),
                                        other => Ok(format!("{}", other)),
                                    })
                                    .collect::<Result<_, anyhow::Error>>()?;
                                result.push(cols);
                            }
                            _ => {
                                return Err(anyhow::anyhow!(
                                    "assert_snapshot rows must be arrays of arrays"
                                ))
                            }
                        }
                    }
                    result
                }
                _ => return Err(anyhow::anyhow!("assert_snapshot 'rows' must be an array")),
            };
            assert_snapshot.push(SnapshotAssert { query, rows });
        }
        if asserts.is_empty()
            && assert_fail.is_empty()
            && assert_notify.is_empty()
            && assert_eq.is_empty()
            && assert_error.is_empty()
            && assert_snapshot.is_empty()
        {
            return Err(anyhow::anyhow!(
                "test '{}' must define at least one assertion type",
                test_name
            ));
        }
        let teardown = match find_attr(body, "teardown") {
            Some(attr) => expr_to_string_vec(attr.expr(), env)?,
            None => Vec::new(),
        };
        Ok(AstTest {
            name: test_name,
            setup,
            asserts,
            assert_fail,
            assert_notify,
            assert_eq,
            assert_error,
            assert_snapshot,
            teardown,
        })
    }

    fn add_to_config(item: Self::Item, config: &mut Config) {
        config.tests.push(item);
    }
}
