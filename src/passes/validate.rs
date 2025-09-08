use anyhow::{bail, Result};

use crate::ir::{Config, EnumSpec};

pub fn validate(cfg: &Config, strict: bool) -> Result<()> {
    for t in &cfg.triggers {
        let fqn = format!(
            "{}.{}",
            t.function_schema.as_deref().unwrap_or("public"),
            t.function
        );
        let found = cfg.functions.iter().any(|f| {
            let fs = f.schema.as_deref().unwrap_or("public");
            let effective_name = f.alt_name.as_deref().unwrap_or(&f.name);
            effective_name == t.function && (t.function_schema.as_deref().unwrap_or(fs) == fs)
        });
        if !found {
            bail!(
                "trigger '{}' references missing function '{}': ensure function exists or set function_schema",
                t.name, fqn
            );
        }
    }

    for t in &cfg.event_triggers {
        let fqn = format!(
            "{}.{}",
            t.function_schema.as_deref().unwrap_or("public"),
            t.function
        );
        let found = cfg.functions.iter().any(|f| {
            let fs = f.schema.as_deref().unwrap_or("public");
            let effective_name = f.alt_name.as_deref().unwrap_or(&f.name);
            effective_name == t.function && (t.function_schema.as_deref().unwrap_or(fs) == fs)
        });
        if !found {
            bail!(
                "event trigger '{}' references missing function '{}': ensure function exists or set function_schema",
                t.name, fqn
            );
        }
    }

    if strict {
        for table in &cfg.tables {
            for column in &table.columns {
                // Check if column type is an enum and if it's defined in HCL
                if is_likely_enum(&column.r#type) {
                    let found_enum =
                        find_enum_for_type(&cfg.enums, &column.r#type, table.schema.as_deref());
                    if found_enum.is_none() {
                        bail!("Strict mode: Enum type '{}' referenced in table '{}' column '{}' is not defined in HCL", column.r#type, table.name, column.name);
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn find_enum_for_type<'a>(
    enums: &'a [EnumSpec],
    coltype: &str,
    table_schema: Option<&str>,
) -> Option<&'a EnumSpec> {
    let t = coltype.to_lowercase();
    let (maybe_schema, name_only) = match t.split_once('.') {
        Some((s, n)) => (Some(s), n),
        None => (None, t.as_str()),
    };
    enums.iter().find(|e| {
        let en = e.name.to_lowercase();
        let es = e.schema.as_deref().unwrap_or("public").to_lowercase();
        if let Some(s) = maybe_schema {
            en == name_only && es == s
        } else {
            // No schema in column type: match by name, and if table has a schema, prefer same-schema enums
            if en == name_only {
                if let Some(ts) = table_schema {
                    es == ts.to_lowercase()
                } else {
                    true
                }
            } else {
                false
            }
        }
    })
}

pub fn is_likely_enum(s: &str) -> bool {
    // Simple heuristic: starts with uppercase letter and contains only alphanumeric characters
    // This is a basic check and might need refinement based on actual enum naming conventions
    s.chars().next().map_or(false, |c| c.is_ascii_uppercase())
        && s.chars().all(|c| c.is_ascii_alphanumeric())
}
