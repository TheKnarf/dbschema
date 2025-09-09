use super::{Backend, CommentStyle, generate_header_comment};
use crate::ir::{ColumnSpec, Config, EnumSpec, TableSpec};
use crate::passes::validate::{find_enum_for_type, is_likely_enum};
use crate::prisma as ps;

use anyhow::{bail, Result};

pub struct PrismaBackend;

impl Backend for PrismaBackend {
    fn name(&self) -> &'static str {
        "prisma"
    }
    fn file_extension(&self) -> &'static str {
        "prisma"
    }
    fn generate(&self, cfg: &Config, strict: bool) -> Result<String> {
        let header = generate_header_comment("Prisma", CommentStyle::Prisma);
        let mut schema = ps::Schema::default();
        for e in &cfg.enums {
            schema.enums.push(enum_to_ast(e));
        }
        for t in &cfg.tables {
            schema.models.push(model_to_ast(t, &cfg.enums, strict)?);
        }
        Ok(format!("{}{}", header, schema.to_string()))
    }
}

fn model_to_ast(t: &TableSpec, enums: &[EnumSpec], strict: bool) -> Result<ps::Model> {
    let model_name = to_model_name(t.alt_name.as_ref().unwrap_or(&t.name));
    let mut model = ps::Model {
        name: model_name,
        fields: Vec::new(),
        attributes: Vec::new(),
    };

    for c in &t.columns {
        let fields = column_to_fields(c, t, enums, strict)?;
        model.fields.extend(fields);
    }

    for br in &t.back_references {
        model.fields.push(ps::Field {
            name: br.name.clone(),
            r#type: ps::Type {
                name: br.table.clone(),
                optional: false,
                list: true,
            },
            attributes: Vec::new(),
        });
    }

    if let Some(pk) = &t.primary_key {
        if pk.columns.len() > 1 {
            model
                .attributes
                .push(ps::BlockAttribute::Id(pk.columns.clone()));
        }
    }

    for ix in &t.indexes {
        if ix.unique {
            if ix.columns.len() > 1 {
                model
                    .attributes
                    .push(ps::BlockAttribute::Unique(ix.columns.clone()));
            }
        } else {
            model
                .attributes
                .push(ps::BlockAttribute::Index(ix.columns.clone()));
        }
    }

    if let Some(table_name) = &t.alt_name {
        model
            .attributes
            .push(ps::BlockAttribute::Map(table_name.clone()));
    } else if let Some(map) = &t.map {
        model
            .attributes
            .push(ps::BlockAttribute::Map(map.clone()));
    }

    Ok(model)
}

fn column_to_fields(
    c: &ColumnSpec,
    t: &TableSpec,
    enums: &[EnumSpec],
    strict: bool,
) -> Result<Vec<ps::Field>> {
    let (ptype, db_attr) = {
        let found_enum = find_enum_for_type(enums, &c.r#type, t.schema.as_deref());
        if let Some(e) = found_enum {
            (e.alt_name.as_deref().unwrap_or(&e.name).to_string(), None)
        } else if strict {
            bail!(
                "Enum type '{}' not found in HCL and strict mode is enabled",
                c.r#type
            );
        } else if is_likely_enum(&c.r#type) {
            (c.r#type.clone(), None)
        } else {
            prisma_type(&c.r#type, c.db_type.as_deref())
        }
    };

    let mut attrs: Vec<ps::FieldAttribute> = Vec::new();

    if let Some(def) = &c.default {
        let dv = if def.trim().eq_ignore_ascii_case("now()") {
            ps::DefaultValue::Now
        } else if def.trim().eq_ignore_ascii_case("uuid_generate_v4()")
            || def.trim().eq_ignore_ascii_case("gen_random_uuid()")
        {
            ps::DefaultValue::Uuid
        } else if def.to_lowercase().contains("nextval(")
            || def.to_lowercase().contains("autoincrement")
        {
            ps::DefaultValue::AutoIncrement
        } else {
            ps::DefaultValue::DbGenerated(def.replace('\\', "\\\\").replace('"', "\\\""))
        };
        attrs.push(ps::FieldAttribute::Default(dv));
    }

    if let Some(pk) = &t.primary_key {
        if pk.columns.len() == 1 && pk.columns[0] == c.name {
            attrs.push(ps::FieldAttribute::Id);
            if is_serial(&c.r#type)
                && !attrs.iter().any(|a| {
                    matches!(
                        a,
                        ps::FieldAttribute::Default(ps::DefaultValue::AutoIncrement)
                    )
                })
            {
                attrs.push(ps::FieldAttribute::Default(ps::DefaultValue::AutoIncrement));
            }
        }
    }

    if t.indexes
        .iter()
        .any(|ix| ix.unique && ix.columns.len() == 1 && ix.columns[0] == c.name)
    {
        attrs.push(ps::FieldAttribute::Unique);
    }

    if let Some(db) = db_attr {
        attrs.push(ps::FieldAttribute::DbNative(db));
    }

    let mut fields = Vec::new();
    fields.push(ps::Field {
        name: c.name.clone(),
        r#type: ps::Type {
            name: ptype,
            optional: c.nullable,
            list: false,
        },
        attributes: attrs,
    });

    if let Some(fk) = t
        .foreign_keys
        .iter()
        .find(|fk| fk.columns.len() == 1 && fk.columns[0] == c.name)
    {
        let rel_attr = ps::RelationAttribute {
            fields: vec![c.name.clone()],
            references: fk.ref_columns.clone(),
            on_delete: fk.on_delete.as_ref().map(|s| map_fk_action(s).to_string()),
            on_update: fk.on_update.as_ref().map(|s| map_fk_action(s).to_string()),
        };
        fields.push(ps::Field {
            name: fk.name.clone().unwrap_or(fk.ref_table.clone()),
            r#type: ps::Type {
                name: to_model_name(&fk.ref_table),
                optional: c.nullable,
                list: false,
            },
            attributes: vec![ps::FieldAttribute::Relation(rel_attr)],
        });
    }

    Ok(fields)
}

fn enum_to_ast(e: &EnumSpec) -> ps::Enum {
    let name = e.alt_name.as_deref().unwrap_or(&e.name).to_string();
    let values = e
        .values
        .iter()
        .map(|v| {
            let (ident, map) = prisma_enum_variant(v);
            ps::EnumValue {
                name: ident,
                mapped_name: map,
            }
        })
        .collect();
    ps::Enum { name, values }
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
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out == db_value {
        (out, None)
    } else {
        (out, Some(db_value.to_string()))
    }
}

fn prisma_type(pg: &str, db_specific: Option<&str>) -> (String, Option<String>) {
    // If we have a specific database type annotation, use it
    if let Some(db_type) = db_specific {
        let dt = db_type.to_lowercase();
        if dt.starts_with("char(") {
            return ("String".into(), Some(format!("@db.Char{}", &db_type[4..])));
        } else if dt.starts_with("varchar(") {
            return (
                "String".into(),
                Some(format!("@db.VarChar{}", &db_type[7..])),
            );
        } else if dt == "text" {
            return ("String".into(), Some("@db.Text".into()));
        } else if dt == "uuid" {
            return ("String".into(), Some("@db.Uuid".into()));
        }
    }

    // Fall back to type-based inference
    let t = pg.to_lowercase();
    match t.as_str() {
        s if s.contains("serial") => ("Int".into(), None),
        "int" | "integer" | "int4" => ("Int".into(), Some("@db.Integer".into())),
        "bigint" | "int8" | "bigserial" => ("BigInt".into(), Some("@db.BigInt".into())),
        s if s.starts_with("varchar") => {
            if let Some(len) = parse_length(s, "varchar(") {
                ("String".into(), Some(format!("@db.VarChar({})", len)))
            } else {
                ("String".into(), None)
            }
        }
        s if s.starts_with("char") => {
            if let Some(len) = parse_length(s, "char(") {
                ("String".into(), Some(format!("@db.Char({})", len)))
            } else {
                ("String".into(), None)
            }
        }
        "text" | "citext" => ("String".into(), None),
        "uuid" => ("String".into(), Some("@db.Uuid".into())),
        "bool" | "boolean" => ("Boolean".into(), None),
        s if s.starts_with("timestamp with time zone") || s == "timestamptz" => {
            ("DateTime".into(), Some("@db.Timestamptz".into()))
        }
        s if s.starts_with("timestamp") => ("DateTime".into(), Some("@db.Timestamp".into())),
        "date" => ("DateTime".into(), Some("@db.Date".into())),
        s if s == "time" || s.starts_with("time ") => ("DateTime".into(), Some("@db.Time".into())),
        "bytea" => ("Bytes".into(), Some("@db.Bytea".into())),
        s if s.starts_with("jsonb") || s == "json" => ("Json".into(), None),
        s if s.starts_with("numeric") || s.starts_with("decimal") => ("Decimal".into(), None),
        "float4" | "real" | "float8" => ("Float".into(), None),
        s if s.contains("double") => ("Float".into(), None),
        _ => (format!("Unsupported(\"{}\")", pg), None),
    }
}

fn parse_length(s: &str, prefix: &str) -> Option<String> {
    if let Some(start) = s.find(prefix) {
        let rest = &s[start + prefix.len()..];
        if let Some(end) = rest.find(')') {
            let len = &rest[..end];
            Some(len.to_string())
        } else {
            None
        }
    } else {
        None
    }
}

fn is_serial(pg: &str) -> bool {
    pg.to_lowercase().contains("serial")
}

fn to_model_name(table: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for ch in table.chars() {
        if ch.is_ascii_alphanumeric() {
            if upper {
                out.push(ch.to_ascii_uppercase());
            } else {
                out.push(ch);
            }
            upper = false;
        } else {
            upper = true;
        }
    }
    if out.is_empty() {
        "Model".into()
    } else {
        out
    }
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
