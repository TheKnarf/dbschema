use std::fmt;

use super::ast::*;

fn write_documentation(f: &mut fmt::Formatter<'_>, doc: &Option<String>) -> fmt::Result {
    if let Some(doc) = doc {
        for line in doc.lines() {
            if line.is_empty() {
                writeln!(f, "///")?;
            } else {
                writeln!(f, "/// {}", line)?;
            }
        }
    }
    Ok(())
}

impl fmt::Display for Schema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut needs_gap = false;

        for block in &self.enums {
            if needs_gap {
                writeln!(f)?;
                writeln!(f)?;
            }
            write!(f, "{}", block)?;
            needs_gap = true;
        }

        for block in &self.models {
            if needs_gap {
                writeln!(f)?;
                writeln!(f)?;
            }
            write!(f, "{}", block)?;
            needs_gap = true;
        }

        for block in &self.views {
            if needs_gap {
                writeln!(f)?;
                writeln!(f)?;
            }
            write!(f, "{}", block)?;
            needs_gap = true;
        }

        for block in &self.composite_types {
            if needs_gap {
                writeln!(f)?;
                writeln!(f)?;
            }
            write!(f, "{}", block)?;
            needs_gap = true;
        }

        for alias in &self.type_aliases {
            if needs_gap {
                writeln!(f)?;
                writeln!(f)?;
            }
            write!(f, "{}", alias)?;
            needs_gap = true;
        }

        for block in &self.custom_blocks {
            if needs_gap {
                writeln!(f)?;
                writeln!(f)?;
            }
            write!(f, "{}", block)?;
            needs_gap = true;
        }

        for block in &self.datasources {
            if needs_gap {
                writeln!(f)?;
                writeln!(f)?;
            }
            write!(f, "{}", block)?;
            needs_gap = true;
        }

        for block in &self.generators {
            if needs_gap {
                writeln!(f)?;
                writeln!(f)?;
            }
            write!(f, "{}", block)?;
            needs_gap = true;
        }

        Ok(())
    }
}

impl fmt::Display for Enum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_documentation(f, &self.documentation)?;
        writeln!(f, "enum {} {{", self.name)?;
        for value in &self.values {
            writeln!(f, "  {}", value)?;
        }
        for attr in &self.attributes {
            writeln!(f, "  {}", attr)?;
        }
        write!(f, "}}")
    }
}

impl fmt::Display for EnumValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_documentation(f, &self.documentation)?;
        write!(f, "{}", self.name)?;
        if let Some(map) = &self.mapped_name {
            write!(f, " @map(\"{}\")", map)?;
        }
        Ok(())
    }
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_documentation(f, &self.documentation)?;
        writeln!(f, "model {} {{", self.name)?;
        for field in &self.fields {
            writeln!(f, "  {}", field)?;
        }
        for attr in &self.attributes {
            writeln!(f, "  {}", attr)?;
        }
        write!(f, "}}")
    }
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_documentation(f, &self.documentation)?;
        writeln!(f, "view {} {{", self.name)?;
        for field in &self.fields {
            writeln!(f, "  {}", field)?;
        }
        for attr in &self.attributes {
            writeln!(f, "  {}", attr)?;
        }
        write!(f, "}}")
    }
}

impl fmt::Display for CompositeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_documentation(f, &self.documentation)?;
        writeln!(f, "type {} {{", self.name)?;
        for field in &self.fields {
            writeln!(f, "  {}", field)?;
        }
        write!(f, "}}")
    }
}

impl fmt::Display for TypeAlias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_documentation(f, &self.documentation)?;
        write!(f, "type {} = {}", self.name, self.target)?;
        for attr in &self.attributes {
            write!(f, " {}", attr)?;
        }
        Ok(())
    }
}

impl fmt::Display for CustomBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_documentation(f, &self.documentation)?;
        if self.contents.starts_with('{') {
            write!(f, "{} {}", self.name, self.contents)?;
        } else {
            write!(f, "{} {{\n{}\n}}", self.name, self.contents)?;
        }
        Ok(())
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_documentation(f, &self.documentation)?;
        write!(f, "{} {}", self.name, self.r#type)?;
        for attr in &self.attributes {
            write!(f, " {}", attr)?;
        }
        Ok(())
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if self.list {
            write!(f, "[]")?;
        }
        if self.optional {
            write!(f, "?")?;
        }
        Ok(())
    }
}

impl fmt::Display for FieldAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldAttribute::Id => write!(f, "@id"),
            FieldAttribute::Unique => write!(f, "@unique"),
            FieldAttribute::Default(value) => write!(f, "@default({})", value),
            FieldAttribute::Relation(rel) => {
                let mut parts = Vec::new();
                if let Some(name) = &rel.name {
                    parts.push(format!("name: \"{}\"", name));
                }
                if !rel.fields.is_empty() {
                    let joined = rel
                        .fields
                        .iter()
                        .map(|f| f.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    parts.push(format!("fields: [{}]", joined));
                }
                if !rel.references.is_empty() {
                    let joined = rel
                        .references
                        .iter()
                        .map(|r| r.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    parts.push(format!("references: [{}]", joined));
                }
                if let Some(map) = &rel.map {
                    parts.push(format!("map: \"{}\"", map));
                }
                if let Some(on_delete) = &rel.on_delete {
                    parts.push(format!("onDelete: {}", on_delete));
                }
                if let Some(on_update) = &rel.on_update {
                    parts.push(format!("onUpdate: {}", on_update));
                }
                if parts.is_empty() {
                    write!(f, "@relation")
                } else {
                    write!(f, "@relation({})", parts.join(", "))
                }
            }
            FieldAttribute::Map(value) => write!(f, "@map(\"{}\")", value),
            FieldAttribute::DbNative(value) => write!(f, "{value}"),
            FieldAttribute::Raw(value) => write!(f, "{value}"),
        }
    }
}

impl fmt::Display for BlockAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockAttribute::Id(columns) => write!(
                f,
                "@@id([{}])",
                columns
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            BlockAttribute::Unique(columns) => write!(
                f,
                "@@unique([{}])",
                columns
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            BlockAttribute::Index(columns) => write!(
                f,
                "@@index([{}])",
                columns
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            BlockAttribute::Map(value) => write!(f, "@@map(\"{}\")", value),
            BlockAttribute::Raw(value) => write!(f, "{value}"),
        }
    }
}

impl fmt::Display for DefaultValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DefaultValue::Now => write!(f, "now()"),
            DefaultValue::Uuid => write!(f, "uuid()"),
            DefaultValue::AutoIncrement => write!(f, "autoincrement()"),
            DefaultValue::DbGenerated(value) => write!(f, "dbgenerated(\"{}\")", value),
            DefaultValue::Expression(expr) => write!(f, "{expr}"),
        }
    }
}

impl fmt::Display for ConfigBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_documentation(f, &self.documentation)?;
        let keyword = match self.kind {
            ConfigBlockKind::Datasource => "datasource",
            ConfigBlockKind::Generator => "generator",
        };
        writeln!(f, "{} {} {{", keyword, self.name)?;
        for property in &self.properties {
            writeln!(f, "  {}", property)?;
        }
        write!(f, "}}")
    }
}

impl fmt::Display for ConfigProperty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_documentation(f, &self.documentation)?;
        if let Some(value) = &self.value {
            write!(f, "{} = {}", self.name, value)
        } else {
            write!(f, "{}", self.name)
        }
    }
}
