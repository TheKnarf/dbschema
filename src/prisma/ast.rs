use std::fmt;

use internment::Intern;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Identifier(Intern<String>);

impl Identifier {
    pub fn new<S: Into<String>>(value: S) -> Self {
        Self(Intern::new(value.into()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl Default for Identifier {
    fn default() -> Self {
        Identifier::new("")
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for Identifier {
    fn from(value: &str) -> Self {
        Identifier::new(value)
    }
}

impl From<String> for Identifier {
    fn from(value: String) -> Self {
        Identifier::new(value)
    }
}

impl From<&Identifier> for String {
    fn from(value: &Identifier) -> Self {
        value.as_str().to_owned()
    }
}

#[derive(Debug, Default, Clone)]
pub struct Schema {
    pub enums: Vec<Enum>,
    pub models: Vec<Model>,
    pub views: Vec<View>,
    pub composite_types: Vec<CompositeType>,
    pub type_aliases: Vec<TypeAlias>,
    pub datasources: Vec<ConfigBlock>,
    pub generators: Vec<ConfigBlock>,
}

#[derive(Debug, Clone)]
pub struct Model {
    pub name: Identifier,
    pub fields: Vec<Field>,
    pub attributes: Vec<BlockAttribute>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct View {
    pub name: Identifier,
    pub fields: Vec<Field>,
    pub attributes: Vec<BlockAttribute>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompositeType {
    pub name: Identifier,
    pub fields: Vec<Field>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TypeAlias {
    pub name: Identifier,
    pub target: Type,
    pub attributes: Vec<FieldAttribute>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigBlockKind {
    Datasource,
    Generator,
}

#[derive(Debug, Clone)]
pub struct ConfigBlock {
    pub kind: ConfigBlockKind,
    pub name: Identifier,
    pub properties: Vec<ConfigProperty>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConfigProperty {
    pub name: Identifier,
    pub value: Option<String>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Enum {
    pub name: Identifier,
    pub values: Vec<EnumValue>,
    pub attributes: Vec<BlockAttribute>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EnumValue {
    pub name: Identifier,
    pub mapped_name: Option<String>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: Identifier,
    pub r#type: Type,
    pub attributes: Vec<FieldAttribute>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Type {
    pub name: String,
    pub optional: bool,
    pub list: bool,
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

#[derive(Debug, Clone)]
pub enum BlockAttribute {
    Id(Vec<Identifier>),
    Unique(Vec<Identifier>),
    Index(Vec<Identifier>),
    Map(String),
    Raw(String),
}

#[derive(Debug, Clone)]
pub enum DefaultValue {
    Now,
    Uuid,
    AutoIncrement,
    DbGenerated(String),
    Expression(String),
}

#[derive(Debug, Clone)]
pub struct RelationAttribute {
    pub name: Option<String>,
    pub fields: Vec<Identifier>,
    pub references: Vec<Identifier>,
    pub map: Option<String>,
    pub on_delete: Option<String>,
    pub on_update: Option<String>,
}
