use anyhow::Result;

use crate::model::{Config, ExtensionSpec, FunctionSpec, TriggerSpec, TableSpec, IndexSpec, ForeignKeySpec, ViewSpec, MaterializedViewSpec, EnumSpec, SchemaSpec, PolicySpec};
use super::Backend;

pub struct PostgresBackend;

impl Backend for PostgresBackend {
    fn name(&self) -> &'static str { "postgres" }
    fn file_extension(&self) -> &'static str { "sql" }
    fn generate(&self, cfg: &Config, _env: &crate::model::EnvVars) -> Result<String> {
        to_sql(cfg)
    }
}

pub fn to_sql(cfg: &Config) -> Result<String> {
    let mut out = String::new();

    // Schemas first
    for s in &cfg.schemas {
        out.push_str(&render_schema(s));
        out.push_str("\n\n");
    }

    // Extensions first
    for e in &cfg.extensions {
        out.push_str(&render_extension(e));
        out.push_str("\n\n");
    }

    // Enums next (types used by tables)
    for e in &cfg.enums {
        out.push_str(&render_enum(e));
        out.push_str("\n\n");
    }

    // Tables next
    for t in &cfg.tables {
        out.push_str(&render_table(t));
        out.push_str("\n\n");
        // Indexes after table creation
        for idx in &t.indexes {
            out.push_str(&render_index(t, idx));
            out.push_str("\n\n");
        }
    }

    // Policies (row-level security) after tables
    for p in &cfg.policies {
        out.push_str(&render_policy(p));
        out.push_str("\n\n");
    }

    for f in &cfg.functions {
        out.push_str(&render_function(f));
        out.push_str("\n\n");
    }

    // Views after functions (they may use functions) and before triggers
    for v in &cfg.views {
        out.push_str(&render_view(v));
        out.push_str("\n\n");
    }

    // Materialized views after views
    for mv in &cfg.materialized {
        out.push_str(&render_materialized(mv));
        out.push_str("\n\n");
    }

    for t in &cfg.triggers {
        out.push_str(&render_trigger(t));
        out.push_str("\n\n");
    }

    Ok(out)
}

fn render_extension(e: &ExtensionSpec) -> String {
    let mut s = String::from("CREATE EXTENSION ");
    if e.if_not_exists {
        s.push_str("IF NOT EXISTS ");
    }
    s.push_str(&ident(&e.name));
    let mut with_parts = Vec::new();
    if let Some(schema) = &e.schema {
        with_parts.push(format!("SCHEMA {}", ident(schema)));
    }
    if let Some(version) = &e.version {
        // version is a literal string
        with_parts.push(format!("VERSION {}", literal(version)));
    }
    if !with_parts.is_empty() {
        s.push_str(" WITH ");
        s.push_str(&with_parts.join(" "));
    }
    s.push(';');
    s
}

fn render_schema(s: &SchemaSpec) -> String {
    let mut stmt = String::from("CREATE ");
    if s.if_not_exists { stmt.push_str("SCHEMA IF NOT EXISTS "); } else { stmt.push_str("SCHEMA "); }
    stmt.push_str(&ident(&s.name));
    if let Some(auth) = &s.authorization {
        stmt.push_str(" AUTHORIZATION ");
        stmt.push_str(&ident(auth));
    }
    stmt.push(';');
    stmt
}

fn render_enum(e: &EnumSpec) -> String {
    let schema = e.schema.as_deref().unwrap_or("public");
    let values = e
        .values
        .iter()
        .map(|v| literal(v))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "DO $$\nBEGIN\n  IF NOT EXISTS (\n    SELECT 1 FROM pg_type t\n    JOIN pg_namespace n ON n.oid = t.typnamespace\n    WHERE t.typname = {name_lit}\n      AND n.nspname = {schema_lit}\n  ) THEN\n    CREATE TYPE {schema_ident}.{name_ident} AS ENUM ({values});\n  END IF;\nEND$$;",
        schema_lit = literal(schema),
        name_lit = literal(&e.name),
        schema_ident = ident(schema),
        name_ident = ident(&e.name),
        values = values,
    )
}

fn render_function(f: &FunctionSpec) -> String {
    let schema = f.schema.as_deref().unwrap_or("public");
    let definer = if f.security_definer { " SECURITY DEFINER" } else { "" };
    let or_replace = if f.replace { "OR REPLACE " } else { "" };
    let lang = f.language.to_lowercase();
    format!(
        "CREATE {or_replace}FUNCTION {schema}.{name}() RETURNS {returns} LANGUAGE {lang}{definer} AS $$\n{body}\n$$;",
        schema = ident(schema),
        name = ident(&f.name),
        returns = f.returns,
        lang = lang,
        definer = definer,
        body = f.body
    )
}

fn render_view(v: &ViewSpec) -> String {
    let schema = v.schema.as_deref().unwrap_or("public");
    let or_replace = if v.replace { "OR REPLACE " } else { "" };
    format!(
        "CREATE {or_replace}VIEW {schema}.{name} AS\n{body};",
        or_replace = or_replace,
        schema = ident(schema),
        name = ident(&v.name),
        body = v.sql
    )
}

fn render_materialized(mv: &MaterializedViewSpec) -> String {
    let schema = mv.schema.as_deref().unwrap_or("public");
    let with = if mv.with_data { "WITH DATA" } else { "WITH NO DATA" };
    format!(
        "DO $$\nBEGIN\n  IF NOT EXISTS (\n    SELECT 1 FROM pg_matviews WHERE schemaname = {schema_lit} AND matviewname = {name_lit}\n  ) THEN\n    CREATE MATERIALIZED VIEW {schema_ident}.{name_ident} AS\n{body}\n    {with};\n  END IF;\nEND$$;",
        schema_lit = literal(schema),
        name_lit = literal(&mv.name),
        schema_ident = ident(schema),
        name_ident = ident(&mv.name),
        body = mv.sql,
        with = with,
    )
}

fn render_table(t: &TableSpec) -> String {
    let schema = t.schema.as_deref().unwrap_or("public");
    let mut lines: Vec<String> = Vec::new();
    for c in &t.columns {
        let mut l = format!("{} {}", ident(&c.name), c.r#type);
        if !c.nullable { l.push_str(" NOT NULL"); }
        if let Some(d) = &c.default { l.push_str(&format!(" DEFAULT {}", d)); }
        lines.push(l);
    }
    if let Some(pk) = &t.primary_key {
        let cols = pk.columns.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ");
        match &pk.name {
            Some(n) => lines.push(format!("CONSTRAINT {} PRIMARY KEY ({})", ident(n), cols)),
            None => lines.push(format!("PRIMARY KEY ({})", cols)),
        }
    }
    for fk in &t.foreign_keys {
        lines.push(render_fk_inline(fk));
    }
    let body = lines
        .into_iter()
        .map(|l| format!("  {}", l))
        .collect::<Vec<_>>()
        .join(",\n");
    let ine = if t.if_not_exists { " IF NOT EXISTS" } else { "" };
    format!(
        "CREATE TABLE{ine} {schema}.{name} (\n{body}\n);",
        ine = ine,
        schema = ident(schema),
        name = ident(&t.name),
        body = body,
    )
}

fn render_fk_inline(fk: &ForeignKeySpec) -> String {
    let cols = fk.columns.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ");
    let ref_schema = fk.ref_schema.as_deref().unwrap_or("public");
    let ref_cols = fk.ref_columns.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ");
    let mut s = String::new();
    if let Some(n) = &fk.name { s.push_str(&format!("CONSTRAINT {} ", ident(n))); }
    s.push_str(&format!(
        "FOREIGN KEY ({cols}) REFERENCES {rschema}.{rtable} ({rcols})",
        cols = cols,
        rschema = ident(ref_schema),
        rtable = ident(&fk.ref_table),
        rcols = ref_cols,
    ));
    if let Some(od) = &fk.on_delete { s.push_str(&format!(" ON DELETE {}", od)); }
    if let Some(ou) = &fk.on_update { s.push_str(&format!(" ON UPDATE {}", ou)); }
    s
}

fn render_index(t: &TableSpec, idx: &IndexSpec) -> String {
    let schema = t.schema.as_deref().unwrap_or("public");
    let cols = idx.columns.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ");
    let unique = if idx.unique { "UNIQUE " } else { "" };
    let name = match &idx.name {
        Some(n) => ident(n),
        None => {
            // derive name: <table>_<col1>_<col2>_idx/uniq
            let mut n = format!("{}_{}_{}", t.name, idx.columns.join("_"), if idx.unique { "uniq" } else { "idx" });
            n = n.replace('.', "_");
            ident(&n)
        }
    };
    format!(
        "CREATE {unique}INDEX IF NOT EXISTS {name} ON {schema}.{table} ({cols});",
        unique = unique,
        name = name,
        schema = ident(schema),
        table = ident(&t.name),
        cols = cols,
    )
}

fn render_trigger(t: &TriggerSpec) -> String {
    let schema = t.schema.as_deref().unwrap_or("public");
    let fn_schema = t.function_schema.as_deref().unwrap_or(schema);
    let timing = t.timing.to_uppercase();
    let events = t
        .events
        .iter()
        .map(|e| e.to_uppercase())
        .collect::<Vec<_>>()
        .join(" OR ");
    let for_each = t.level.to_uppercase();
    let when = t
        .when
        .as_ref()
        .map(|w| format!("\n    WHEN ({w})"))
        .unwrap_or_default();

    format!(
        "DO $$\nBEGIN\n  IF NOT EXISTS (\n    SELECT 1 FROM pg_trigger tg\n    JOIN pg_class c ON c.oid = tg.tgrelid\n    JOIN pg_namespace n ON n.oid = c.relnamespace\n    WHERE tg.tgname = {tgname}\n      AND n.nspname = {schema_lit}\n      AND c.relname = {table_lit}\n  ) THEN\n    CREATE TRIGGER {tg}\n    {timing} {events} ON {schema_ident}.{table_ident}\n    FOR EACH {for_each}{when}\n    EXECUTE FUNCTION {fn_schema_ident}.{fn_name}();\n  END IF;\nEND$$;",
        tgname = literal(&t.name),
        schema_lit = literal(schema),
        table_lit = literal(&t.table),
        tg = ident(&t.name),
        timing = timing,
        events = events,
        for_each = for_each,
        when = when,
        schema_ident = ident(schema),
        table_ident = ident(&t.table),
        fn_schema_ident = ident(fn_schema),
        fn_name = ident(&t.function)
    )
}

fn render_policy(p: &PolicySpec) -> String {
    let schema = p.schema.as_deref().unwrap_or("public");
    let cmd = p.command.to_uppercase();
    let as_clause = match p.r#as.as_ref().map(|s| s.to_uppercase()) {
        Some(ref k) if k == "PERMISSIVE" || k == "RESTRICTIVE" => format!(" AS {}", k),
        _ => String::new(),
    };
    let for_clause = if cmd == "ALL" { String::new() } else { format!(" FOR {}", cmd) };
    let to_clause = if p.roles.is_empty() {
        String::new()
    } else {
        let roles = p.roles.iter().map(|r| ident(r)).collect::<Vec<_>>().join(", ");
        format!(" TO {}", roles)
    };
    let using_clause = match &p.using {
        Some(u) => format!("\n    USING ({})", u),
        None => String::new(),
    };
    let check_clause = match &p.check {
        Some(c) => format!("\n    WITH CHECK ({})", c),
        None => String::new(),
    };

    format!(
        "DO $$\nBEGIN\n  IF NOT EXISTS (\n    SELECT 1 FROM pg_policies\n    WHERE policyname = {pname}\n      AND schemaname = {schema_lit}\n      AND tablename = {table_lit}\n  ) THEN\n    CREATE POLICY {pname_ident} ON {schema_ident}.{table_ident}{as_clause}{for_clause}{to_clause}{using}{check};\n  END IF;\nEND$$;",
        pname = literal(&p.name),
        schema_lit = literal(schema),
        table_lit = literal(&p.table),
        pname_ident = ident(&p.name),
        schema_ident = ident(schema),
        table_ident = ident(&p.table),
        as_clause = as_clause,
        for_clause = for_clause,
        to_clause = to_clause,
        using = using_clause,
        check = check_clause,
    )
}

fn ident(s: &str) -> String {
    let escaped = s.replace('"', "\"");
    format!("\"{}\"", escaped)
}

fn literal(s: &str) -> String {
    let escaped = s.replace("'", "''");
    format!("'{}'", escaped)
}
