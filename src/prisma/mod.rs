use anyhow::{anyhow, Result};
use psl::schema_ast::ast::{self, WithAttributes, WithDocumentation, WithIdentifier};
use std::fmt;

#[derive(Debug, Default, Clone)]
pub struct Schema {
    pub enums: Vec<Enum>,
    pub models: Vec<Model>,
    pub views: Vec<View>,
    pub composite_types: Vec<CompositeType>,
    pub datasources: Vec<ConfigBlock>,
    pub generators: Vec<ConfigBlock>,
}

impl fmt::Display for Schema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut needs_gap = false;

        for enm in &self.enums {
            if needs_gap {
                writeln!(f)?;
                writeln!(f)?;
            }
            write!(f, "{}", enm)?;
            needs_gap = true;
        }

        for model in &self.models {
            if needs_gap {
                writeln!(f)?;
                writeln!(f)?;
            }
            write!(f, "{}", model)?;
            needs_gap = true;
        }

        for view in &self.views {
            if needs_gap {
                writeln!(f)?;
                writeln!(f)?;
            }
            write!(f, "{}", view)?;
            needs_gap = true;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CompositeType {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct ConfigBlock {
    pub name: String,
    pub properties: Vec<ConfigProperty>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConfigProperty {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Enum {
    pub name: String,
    pub values: Vec<EnumValue>,
    pub attributes: Vec<BlockAttribute>,
}

impl fmt::Display for Enum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "enum {} {{", self.name)?;
        for v in &self.values {
            writeln!(f, "  {}", v)?;
        }
        for attr in &self.attributes {
            writeln!(f, "  {}", attr)?;
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
pub struct View {
    pub name: String,
    pub fields: Vec<Field>,
    pub attributes: Vec<BlockAttribute>,
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "view {} {{", self.name)?;
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
                let mut parts = Vec::new();
                if let Some(name) = &r.name {
                    parts.push(format!("name: \"{}\"", name));
                }
                if !r.fields.is_empty() {
                    parts.push(format!("fields: [{}]", r.fields.join(", ")));
                }
                if !r.references.is_empty() {
                    parts.push(format!("references: [{}]", r.references.join(", ")));
                }
                if let Some(map) = &r.map {
                    parts.push(format!("map: \"{}\"", map));
                }
                if let Some(od) = &r.on_delete {
                    parts.push(format!("onDelete: {}", od));
                }
                if let Some(ou) = &r.on_update {
                    parts.push(format!("onUpdate: {}", ou));
                }
                if parts.is_empty() {
                    write!(f, "@relation")
                } else {
                    write!(f, "@relation({})", parts.join(", "))
                }
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
    pub name: Option<String>,
    pub fields: Vec<String>,
    pub references: Vec<String>,
    pub map: Option<String>,
    pub on_delete: Option<String>,
    pub on_update: Option<String>,
}

pub fn parse_schema_str(input: &str) -> Result<Schema> {
    let validated = psl::parse_schema_without_extensions(input).map_err(|err| anyhow!(err))?;
    let ast = validated.db.ast_assert_single();
    Ok(convert_schema(ast))
}

fn convert_schema(ast: &ast::SchemaAst) -> Schema {
    let mut schema = Schema::default();

    for top in &ast.tops {
        match top {
            ast::Top::Model(model) => {
                if model.is_view() {
                    schema.views.push(convert_view(model));
                } else {
                    schema.models.push(convert_model(model));
                }
            }
            ast::Top::Enum(enm) => {
                schema.enums.push(convert_enum(enm));
            }
            ast::Top::CompositeType(ct) => {
                schema.composite_types.push(convert_composite_type(ct));
            }
            ast::Top::Source(source) => {
                schema.datasources.push(convert_config_block(source));
            }
            ast::Top::Generator(generator) => {
                schema.generators.push(convert_generator_block(generator));
            }
        }
    }

    schema
}

fn convert_model(model: &ast::Model) -> Model {
    let (name, fields, attributes) = convert_model_parts(model);
    Model {
        name,
        fields,
        attributes,
    }
}

fn convert_view(model: &ast::Model) -> View {
    let (name, fields, attributes) = convert_model_parts(model);
    View {
        name,
        fields,
        attributes,
    }
}

fn convert_model_parts(model: &ast::Model) -> (String, Vec<Field>, Vec<BlockAttribute>) {
    let fields = model
        .iter_fields()
        .map(|(_, field)| convert_field(field))
        .collect();
    let attributes = model
        .attributes()
        .iter()
        .map(convert_block_attribute)
        .collect();

    (model.identifier().name.clone(), fields, attributes)
}

fn convert_enum(enm: &ast::Enum) -> Enum {
    let values = enm
        .iter_values()
        .map(|(_, value)| convert_enum_value(value))
        .collect();
    let attributes = enm.attributes.iter().map(convert_block_attribute).collect();

    Enum {
        name: enm.identifier().name.clone(),
        values,
        attributes,
    }
}

fn convert_enum_value(value: &ast::EnumValue) -> EnumValue {
    let mapped_name = value
        .attributes
        .iter()
        .find(|attr| attr.name.name == "map")
        .and_then(|attr| attr.arguments.arguments.iter().next())
        .and_then(|arg| extract_string(&arg.value));

    EnumValue {
        name: value.name.name.clone(),
        mapped_name,
    }
}

fn convert_composite_type(ct: &ast::CompositeType) -> CompositeType {
    let fields = ct
        .iter_fields()
        .map(|(_, field)| convert_field(field))
        .collect();

    CompositeType {
        name: ct.identifier().name.clone(),
        fields,
    }
}

fn convert_field(field: &ast::Field) -> Field {
    let r#type = convert_type(&field.field_type, field.arity);
    let attributes = field
        .attributes
        .iter()
        .map(convert_field_attribute)
        .collect();

    Field {
        name: field.name().to_string(),
        r#type,
        attributes,
    }
}

fn convert_type(field_type: &ast::FieldType, arity: ast::FieldArity) -> Type {
    let name = match field_type {
        ast::FieldType::Supported(identifier) => identifier.name.clone(),
        ast::FieldType::Unsupported(value, _) => format!("Unsupported(\"{}\")", value),
    };

    Type {
        name,
        optional: matches!(arity, ast::FieldArity::Optional),
        list: matches!(arity, ast::FieldArity::List),
    }
}

fn convert_field_attribute(attr: &ast::Attribute) -> FieldAttribute {
    let name = attr.name.name.as_str();
    match name {
        "id" => FieldAttribute::Id,
        "unique" => FieldAttribute::Unique,
        "default" => attr
            .arguments
            .arguments
            .iter()
            .next()
            .map(|arg| FieldAttribute::Default(convert_default_value(&arg.value)))
            .unwrap_or_else(|| FieldAttribute::Raw(attribute_to_string(attr, "@"))),
        "map" => attr
            .arguments
            .arguments
            .iter()
            .next()
            .and_then(|arg| extract_string(&arg.value))
            .map(FieldAttribute::Map)
            .unwrap_or_else(|| FieldAttribute::Raw(attribute_to_string(attr, "@"))),
        "relation" => convert_relation_attribute(attr)
            .unwrap_or_else(|| FieldAttribute::Raw(attribute_to_string(attr, "@"))),
        name if name.starts_with("db.") => FieldAttribute::DbNative(attribute_to_string(attr, "@")),
        _ => FieldAttribute::Raw(attribute_to_string(attr, "@")),
    }
}

fn convert_block_attribute(attr: &ast::Attribute) -> BlockAttribute {
    let name = attr.name.name.as_str();
    match name {
        "id" => BlockAttribute::Id(extract_fields_argument(&attr.arguments)),
        "unique" => BlockAttribute::Unique(extract_fields_argument(&attr.arguments)),
        "index" => BlockAttribute::Index(extract_fields_argument(&attr.arguments)),
        "map" => attr
            .arguments
            .arguments
            .iter()
            .next()
            .and_then(|arg| extract_string(&arg.value))
            .map(BlockAttribute::Map)
            .unwrap_or_else(|| BlockAttribute::Raw(attribute_to_string(attr, "@@"))),
        _ => BlockAttribute::Raw(attribute_to_string(attr, "@@")),
    }
}

fn convert_default_value(expr: &ast::Expression) -> DefaultValue {
    match expr {
        ast::Expression::Function(name, args, _) => match name.as_str() {
            "now" => DefaultValue::Now,
            "autoincrement" => DefaultValue::AutoIncrement,
            "uuid" => DefaultValue::Uuid,
            "dbgenerated" => args
                .arguments
                .iter()
                .next()
                .and_then(|arg| extract_string(&arg.value))
                .map(DefaultValue::DbGenerated)
                .unwrap_or_else(|| DefaultValue::Expression(expr.to_string())),
            _ => DefaultValue::Expression(expr.to_string()),
        },
        ast::Expression::StringValue(_, _)
        | ast::Expression::ConstantValue(_, _)
        | ast::Expression::NumericValue(_, _)
        | ast::Expression::Array(_, _) => DefaultValue::Expression(expr.to_string()),
    }
}

fn convert_relation_attribute(attr: &ast::Attribute) -> Option<FieldAttribute> {
    let mut name = None;
    let mut fields = Vec::new();
    let mut references = Vec::new();
    let mut map = None;
    let mut on_delete = None;
    let mut on_update = None;

    for argument in &attr.arguments.arguments {
        match argument.name() {
            Some("fields") => {
                fields = extract_string_array(&argument.value);
            }
            Some("references") => {
                references = extract_string_array(&argument.value);
            }
            Some("name") => {
                name = extract_string(&argument.value).or_else(|| Some(argument.value.to_string()));
            }
            Some("map") => {
                map = extract_string(&argument.value).or_else(|| Some(argument.value.to_string()));
            }
            Some("onDelete") => {
                on_delete = Some(argument.value.to_string());
            }
            Some("onUpdate") => {
                on_update = Some(argument.value.to_string());
            }
            None => match &argument.value {
                ast::Expression::Array(_, _) => {
                    if fields.is_empty() {
                        fields = extract_string_array(&argument.value);
                    } else if references.is_empty() {
                        references = extract_string_array(&argument.value);
                    }
                }
                _ => {
                    if name.is_none() {
                        name = extract_string(&argument.value)
                            .or_else(|| Some(argument.value.to_string()));
                    } else if map.is_none() {
                        map = extract_string(&argument.value)
                            .or_else(|| Some(argument.value.to_string()));
                    }
                }
            },
            _ => {}
        }
    }

    Some(FieldAttribute::Relation(RelationAttribute {
        name,
        fields,
        references,
        map,
        on_delete,
        on_update,
    }))
}

fn convert_config_block(source: &ast::SourceConfig) -> ConfigBlock {
    ConfigBlock {
        name: source.identifier().name.clone(),
        properties: source
            .properties
            .iter()
            .map(convert_config_property)
            .collect(),
        documentation: source.documentation().map(str::to_string),
    }
}

fn convert_generator_block(generator: &ast::GeneratorConfig) -> ConfigBlock {
    ConfigBlock {
        name: generator.identifier().name.clone(),
        properties: generator
            .properties
            .iter()
            .map(convert_config_property)
            .collect(),
        documentation: generator.documentation().map(str::to_string),
    }
}

fn convert_config_property(prop: &ast::ConfigBlockProperty) -> ConfigProperty {
    ConfigProperty {
        name: prop.identifier().name.clone(),
        value: prop.value.as_ref().map(|value| value.to_string()),
    }
}

fn extract_fields_argument(arguments: &ast::ArgumentsList) -> Vec<String> {
    arguments
        .arguments
        .iter()
        .find(|arg| arg.is_unnamed() || arg.name() == Some("fields"))
        .map(|arg| extract_string_array(&arg.value))
        .unwrap_or_default()
}

fn extract_string(expr: &ast::Expression) -> Option<String> {
    match expr {
        ast::Expression::StringValue(value, _) => Some(value.clone()),
        _ => None,
    }
}

fn extract_string_array(expr: &ast::Expression) -> Vec<String> {
    expr.as_array()
        .map(|(values, _)| values.iter().map(|value| value.to_string()).collect())
        .unwrap_or_default()
}

fn attribute_to_string(attr: &ast::Attribute, prefix: &str) -> String {
    let mut out = format!("{}{}", prefix, attr.name.name);
    let args = format_arguments(&attr.arguments);
    if !args.is_empty() {
        out.push('(');
        out.push_str(&args);
        out.push(')');
    }
    out
}

fn format_arguments(arguments: &ast::ArgumentsList) -> String {
    let mut parts: Vec<String> = arguments
        .arguments
        .iter()
        .map(|arg| arg.to_string())
        .collect();
    parts.extend(
        arguments
            .empty_arguments
            .iter()
            .map(|empty| format!("{}:", empty.name.name)),
    );
    let mut joined = parts.join(", ");
    if arguments.trailing_comma.is_some() && !joined.is_empty() {
        joined.push(',');
    }
    joined
}
