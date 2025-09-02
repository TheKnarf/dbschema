use std::fmt;

#[derive(Debug, Default, Clone)]
pub struct Schema {
    pub enums: Vec<Enum>,
    pub models: Vec<Model>,
}

impl fmt::Display for Schema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for e in &self.enums {
            if !first {
                writeln!(f)?;
                writeln!(f)?;
            }
            write!(f, "{}", e)?;
            first = false;
        }
        if !self.enums.is_empty() && !self.models.is_empty() {
            writeln!(f)?;
            writeln!(f)?;
        }
        for (i, m) in self.models.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
                writeln!(f)?;
            }
            write!(f, "{}", m)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Enum {
    pub name: String,
    pub values: Vec<EnumValue>,
}

impl fmt::Display for Enum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "enum {} {{", self.name)?;
        for v in &self.values {
            writeln!(f, "  {}", v)?;
        }
        writeln!(f, "}}")
    }
}

#[derive(Debug, Clone)]
pub struct EnumValue {
    pub name: String,
    pub mapped_name: Option<String>,
}

impl fmt::Display for EnumValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(map) = &self.mapped_name {
            write!(f, " @map(\"{}\")", map)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Model {
    pub name: String,
    pub fields: Vec<Field>,
    pub attributes: Vec<BlockAttribute>,
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "model {} {{", self.name)?;
        for field in &self.fields {
            writeln!(f, "  {}", field)?;
        }
        for attr in &self.attributes {
            writeln!(f, "  {}", attr)?;
        }
        writeln!(f, "}}")
    }
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub r#type: Type,
    pub attributes: Vec<FieldAttribute>,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.name, self.r#type)?;
        for attr in &self.attributes {
            write!(f, " {}", attr)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Type {
    pub name: String,
    pub optional: bool,
    pub list: bool,
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

#[derive(Debug, Clone)]
pub enum FieldAttribute {
    Id,
    Unique,
    Default(DefaultValue),
    Relation(RelationAttribute),
    Map(String),
    DbNative(String),
    Raw(String),
}

impl fmt::Display for FieldAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldAttribute::Id => write!(f, "@id"),
            FieldAttribute::Unique => write!(f, "@unique"),
            FieldAttribute::Default(d) => write!(f, "@default({})", d),
            FieldAttribute::Relation(r) => {
                write!(
                    f,
                    "@relation(fields: [{}], references: [{}]",
                    r.fields.join(", "),
                    r.references.join(", ")
                )?;
                if let Some(od) = &r.on_delete {
                    write!(f, ", onDelete: {}", od)?;
                }
                if let Some(ou) = &r.on_update {
                    write!(f, ", onUpdate: {}", ou)?;
                }
                write!(f, ")")
            }
            FieldAttribute::Map(m) => write!(f, "@map(\"{}\")", m),
            FieldAttribute::DbNative(s) => write!(f, "{}", s),
            FieldAttribute::Raw(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone)]
pub enum BlockAttribute {
    Id(Vec<String>),
    Unique(Vec<String>),
    Index(Vec<String>),
    Map(String),
    Raw(String),
}

impl fmt::Display for BlockAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockAttribute::Id(cols) => write!(f, "@@id([{}])", cols.join(", ")),
            BlockAttribute::Unique(cols) => write!(f, "@@unique([{}])", cols.join(", ")),
            BlockAttribute::Index(cols) => write!(f, "@@index([{}])", cols.join(", ")),
            BlockAttribute::Map(name) => write!(f, "@@map(\"{}\")", name),
            BlockAttribute::Raw(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DefaultValue {
    Now,
    Uuid,
    AutoIncrement,
    DbGenerated(String),
    Expression(String),
}

impl fmt::Display for DefaultValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DefaultValue::Now => write!(f, "now()"),
            DefaultValue::Uuid => write!(f, "uuid()"),
            DefaultValue::AutoIncrement => write!(f, "autoincrement()"),
            DefaultValue::DbGenerated(s) => write!(f, "dbgenerated(\"{}\")", s),
            DefaultValue::Expression(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RelationAttribute {
    pub fields: Vec<String>,
    pub references: Vec<String>,
    pub on_delete: Option<String>,
    pub on_update: Option<String>,
}
