use crate::parser::{Config, ExtensionSpec, FunctionSpec, TriggerSpec};
use anyhow::Result;

pub fn to_sql(cfg: &Config) -> Result<String> {
    let mut out = String::new();

    // Extensions first
    for e in &cfg.extensions {
        out.push_str(&render_extension(e));
        out.push_str("\n\n");
    }

    for f in &cfg.functions {
        out.push_str(&render_function(f));
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

fn ident(s: &str) -> String {
    let escaped = s.replace('"', "\"");
    format!("\"{}\"", escaped)
}

fn literal(s: &str) -> String {
    let escaped = s.replace("'", "''");
    format!("'{}'", escaped)
}
