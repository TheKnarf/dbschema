use anyhow::Result;

use super::Backend;
use crate::model::{Config, TableSpec, ColumnSpec, EnumSpec};

pub struct PrismaBackend;

impl Backend for PrismaBackend {
    fn name(&self) -> &'static str { "prisma" }
    fn file_extension(&self) -> &'static str { "prisma" }
    fn generate(&self, cfg: &Config) -> Result<String> {
        let mut out = String::new();
        // Output only enums and models; generator/datasource are managed externally.

        // Enums first so models can refer to them
        for e in &cfg.enums {
            out.push_str(&render_enum(e));
            out.push_str("\n\n");
        }

        for t in &cfg.tables {
            out.push_str(&render_model(t, &cfg.enums));
            out.push_str("\n\n");
        }
        Ok(out)
    }
}

fn render_model(t: &TableSpec, enums: &[EnumSpec]) -> String {
    let mut s = String::new();
    let model_name = to_model_name(&t.name);
    s.push_str(&format!("model {} {{\n", model_name));

    // columns → fields
    for c in &t.columns {
        s.push_str("  ");
        s.push_str(&render_field(c, t, enums));
        s.push_str("\n");
    }

    // primary key
    if let Some(pk) = &t.primary_key {
        if pk.columns.len() == 1 {
            // Ensure field has @id (added in render_field if matches)
        } else {
            let cols = pk.columns.join(", ");
            s.push_str(&format!("  @@id([{}])\n", cols));
        }
    }

    // indexes
    for ix in &t.indexes {
        let col_list = ix.columns.join(", ");
        if ix.unique {
            // Skip single-column uniques; they are added as @unique on the field
            if ix.columns.len() > 1 {
                s.push_str(&format!("  @@unique([{}])\n", col_list));
            }
        } else {
            s.push_str(&format!("  @@index([{}])\n", col_list));
        }
    }

    // Map model to table name if model name differs
    if model_name != t.name {
        s.push_str(&format!("  @@map(\"{}\")\n", t.name));
    }

    s.push_str("}\n");
    s
}

fn render_field(c: &ColumnSpec, t: &TableSpec, enums: &[EnumSpec]) -> String {
    let mut parts: Vec<String> = Vec::new();
    // name
    parts.push(c.name.clone());
    // type + nullability
    let (ptype, db_attr) = match find_enum_for_type(enums, &c.r#type, t.schema.as_deref()) {
        Some(e) => (e.name.clone(), None),
        None => prisma_type(&c.r#type, c.db_type.as_deref()),
    };
    let type_with_null = if c.nullable { format!("{}?", ptype) } else { ptype };
    parts.push(type_with_null);

    // default
    if let Some(def) = &c.default {
        if def.trim().eq_ignore_ascii_case("now()") {
            parts.push("@default(now())".into());
        } else if def.trim().eq_ignore_ascii_case("uuid_generate_v4()") || def.trim().eq_ignore_ascii_case("gen_random_uuid()") {
            parts.push("@default(uuid())".into());
        } else if def.to_lowercase().contains("nextval(") || def.to_lowercase().contains("autoincrement") {
            parts.push("@default(autoincrement())".into());
        } else {
            parts.push(format!("@default(dbgenerated(\"{}\"))", def.replace('\\', "\\\\").replace('"', "\\\"")));
        }
    }

    // primary key single column
    if let Some(pk) = &t.primary_key {
        if pk.columns.len() == 1 && pk.columns[0] == c.name {
            parts.push("@id".into());
            // If type suggests auto-increment, add it if not already
            if is_serial(&c.r#type) && !parts.iter().any(|p| p.contains("@default(autoincrement())")) {
                parts.push("@default(autoincrement())".into());
            }
        }
    }

    // unique single-column indexes → @unique
    if t.indexes.iter().any(|ix| ix.unique && ix.columns.len() == 1 && ix.columns[0] == c.name) {
        parts.push("@unique".into());
    }

    // foreign key relations: add a separate relation field below the scalar?
    // Keep scalar field here. We'll append relation fields after columns.

    // native db type attribute
    if let Some(db) = db_attr {
        parts.push(db);
    }

    let mut line = parts.join(" ");

    // After scalar field line, optionally add relation field lines for FKs on this column
    // Collect relation lines and append after (on separate lines) by returning combined string with \n  prefix in caller.
    if let Some(fk) = t.foreign_keys.iter().find(|fk| fk.columns.len() == 1 && fk.columns[0] == c.name) {
        let ref_model = to_model_name(&fk.ref_table);
        let rel_field_name = fk.ref_table.clone();
        let mut rel = format!("\n  {} {}? @relation(fields: [{}], references: [{}]", rel_field_name, ref_model, c.name, fk.ref_columns.join(", "));
        if let Some(od) = &fk.on_delete { rel.push_str(&format!(", onDelete: {}", map_fk_action(od))); }
        if let Some(ou) = &fk.on_update { rel.push_str(&format!(", onUpdate: {}", map_fk_action(ou))); }
        rel.push(')');
        line.push_str(&rel);
    }

    line
}

fn render_enum(e: &EnumSpec) -> String {
    // Keep enum name as DB name to avoid relying on @@map on enums.
    let mut s = String::new();
    s.push_str(&format!("enum {} {{\n", e.name));
    for v in &e.values {
        let (ident, map) = prisma_enum_variant(v);
        if let Some(mapattr) = map {
            s.push_str(&format!("  {} {}\n", ident, mapattr));
        } else {
            s.push_str(&format!("  {}\n", ident));
        }
    }
    s.push_str("}\n");
    s
}

fn prisma_enum_variant(db_value: &str) -> (String, Option<String>) {
    // Prisma enum value must match [A-Za-z_][A-Za-z0-9_]*
    let mut out = String::new();
    let mut chars = db_value.chars();
    if let Some(first) = chars.next() {
        if first.is_ascii_alphabetic() || first == '_' {
            out.push(first);
        } else {
            out.push('_');
        }
    }
    for ch in chars {
        if ch.is_ascii_alphanumeric() || ch == '_' { out.push(ch); } else { out.push('_'); }
    }
    if out == db_value { (out, None) } else { (out, Some(format!("@map(\"{}\")", db_value))) }
}

fn prisma_type(pg: &str, db_specific: Option<&str>) -> (String, Option<String>) {
    // If we have a specific database type annotation, use it
    if let Some(db_type) = db_specific {
        let dt = db_type.to_uppercase();
        if dt.starts_with("CHAR(") {
            return ("String".into(), Some(format!("@db.Char{}", &db_type[4..])));
        } else if dt.starts_with("VARCHAR(") {
            return ("String".into(), Some(format!("@db.VarChar{}", &db_type[7..])));
        } else if dt == "TEXT" {
            return ("String".into(), Some("@db.Text".into()));
        } else if dt == "UUID" {
            return ("String".into(), Some("@db.Uuid".into()));
        }
    }

    // Fall back to type-based inference
    let t = pg.to_lowercase();
    let (base, db): (String, Option<&str>) = if t.contains("serial") {
        ("Int".into(), None)
    } else if t == "int" || t == "integer" || t == "int4" {
        ("Int".into(), Some("@db.Integer"))
    } else if t == "bigint" || t == "int8" || t == "bigserial" {
        ("BigInt".into(), Some("@db.BigInt"))
    } else if t.starts_with("varchar") || t == "text" || t.starts_with("char") || t == "citext" {
        ("String".into(), None)
    } else if t == "uuid" {
        ("String".into(), Some("@db.Uuid"))
    } else if t == "bool" || t == "boolean" {
        ("Boolean".into(), None)
    } else if t.starts_with("timestamp with time zone") || t == "timestamptz" {
        ("DateTime".into(), Some("@db.Timestamptz"))
    } else if t.starts_with("timestamp") {
        ("DateTime".into(), Some("@db.Timestamp"))
    } else if t == "date" {
        ("DateTime".into(), Some("@db.Date"))
    } else if t == "time" || t.starts_with("time ") {
        ("DateTime".into(), Some("@db.Time"))
    } else if t == "bytea" {
        ("Bytes".into(), Some("@db.Bytea"))
    } else if t.starts_with("jsonb") || t == "json" {
        ("Json".into(), None)
    } else if t.starts_with("numeric") || t.starts_with("decimal") {
        ("Decimal".into(), None)
    } else if t == "float4" || t == "real" || t == "float8" || t.contains("double") {
        ("Float".into(), None)
    } else {
        (format!("Unsupported(\"{}\")", pg), None)
    };
    (base, db.map(|d| d.to_string()))
}

fn is_serial(pg: &str) -> bool {
    pg.to_lowercase().contains("serial")
}

fn to_model_name(table: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for ch in table.chars() {
        if ch.is_ascii_alphanumeric() {
            if upper { out.push(ch.to_ascii_uppercase()); } else { out.push(ch); }
            upper = false;
        } else {
            upper = true;
        }
    }
    if out.is_empty() { "Model".into() } else { out }
}

fn map_fk_action(s: &str) -> &str {
    match s.to_ascii_uppercase().as_str() {
        "CASCADE" => "Cascade",
        "RESTRICT" => "Restrict",
        "SET NULL" => "SetNull",
        "SET DEFAULT" => "SetDefault",
        _ => "NoAction",
    }
}

fn find_enum_for_type<'a>(enums: &'a [EnumSpec], coltype: &str, table_schema: Option<&str>) -> Option<&'a EnumSpec> {
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
                if let Some(ts) = table_schema { es == ts.to_lowercase() } else { true }
            } else { false }
        }
    })
}
