use std::fmt;

use anyhow::{Result, anyhow};
use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
use chumsky::Parser;
use chumsky::Stream;
use chumsky::error::{Simple, SimpleReason};
use chumsky::prelude::{BoxedParser, *};

use super::ast::*;

type Span = std::ops::Range<usize>;
type Spanned<T> = (T, Span);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Token {
    Ident(String),
    String(String),
    Number(String),
    Boolean(bool),
    DocComment(String),
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Equals,
    Dot,
    At,
    AtAt,
    Question,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Ident(ident) => write!(f, "identifier `{ident}`"),
            Token::String(s) => write!(f, "string \"{}\"", s.escape_debug()),
            Token::Number(n) => write!(f, "number {n}"),
            Token::Boolean(b) => write!(f, "boolean {b}"),
            Token::DocComment(_) => write!(f, "documentation comment"),
            Token::LBrace => write!(f, "`{{`"),
            Token::RBrace => write!(f, "`}}`"),
            Token::LParen => write!(f, "`(`"),
            Token::RParen => write!(f, "`)`"),
            Token::LBracket => write!(f, "`[`"),
            Token::RBracket => write!(f, "`]`"),
            Token::Comma => write!(f, "`,`"),
            Token::Colon => write!(f, "`:`"),
            Token::Equals => write!(f, "`=`"),
            Token::Dot => write!(f, "`.`"),
            Token::At => write!(f, "`@`"),
            Token::AtAt => write!(f, "`@@`"),
            Token::Question => write!(f, "`?`"),
        }
    }
}

#[derive(Clone, Debug)]
struct Attribute {
    name: Identifier,
    arguments: Vec<Argument>,
}

#[derive(Clone, Debug)]
struct Argument {
    name: Option<Identifier>,
    value: Value,
}

#[derive(Clone, Debug)]
struct FunctionCall {
    path: Vec<Identifier>,
    arguments: Vec<Argument>,
}

#[derive(Clone, Debug)]
enum Value {
    String(String),
    Number(String),
    Boolean(bool),
    Array(Vec<Value>),
    Function(FunctionCall),
    Path(Vec<Identifier>),
}

impl Value {
    fn to_string(&self) -> String {
        match self {
            Value::String(s) => quote_string(s),
            Value::Number(n) => n.clone(),
            Value::Boolean(b) => b.to_string(),
            Value::Array(values) => {
                let inner = values
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", inner)
            }
            Value::Function(func) => {
                let name = func
                    .path
                    .iter()
                    .map(|segment| segment.to_string())
                    .collect::<Vec<_>>()
                    .join(".");
                let args = func
                    .arguments
                    .iter()
                    .map(|arg| arg.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", name, args)
            }
            Value::Path(path) => path
                .iter()
                .map(|segment| segment.to_string())
                .collect::<Vec<_>>()
                .join("."),
        }
    }
}

impl Argument {
    fn to_string(&self) -> String {
        match &self.name {
            Some(name) => format!("{}: {}", name, self.value.to_string()),
            None => self.value.to_string(),
        }
    }
}

fn quote_string(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    out.push('"');
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

pub fn parse_schema_str(input: &str) -> Result<Schema> {
    let (tokens, lex_errors) = lexer().parse_recovery(input);
    if !lex_errors.is_empty() {
        return Err(render_errors(lex_errors, input));
    }
    let tokens = tokens.ok_or_else(|| anyhow!("failed to tokenize Prisma schema"))?;

    let len = input.len();
    let stream = Stream::from_iter(
        len..len + 1,
        tokens.into_iter().map(|(token, span)| (token, span)),
    );

    let (schema, parse_errors) = schema_parser().parse_recovery(stream);
    if !parse_errors.is_empty() {
        return Err(render_errors(parse_errors, input));
    }

    schema.ok_or_else(|| anyhow!("failed to parse Prisma schema"))
}

fn lexer() -> impl Parser<char, Vec<Spanned<Token>>, Error = Simple<char>> {
    let whitespace = filter(|c: &char| c.is_whitespace()).ignored();
    let block_comment = just("/*")
        .ignore_then(take_until(just("*/")))
        .then_ignore(just("*/"))
        .ignored();
    let line_comment = just("//")
        .then(just('/').not())
        .ignore_then(filter(|c: &char| *c != '\n').repeated())
        .then_ignore(just('\n').or_not())
        .ignored();
    let skip = choice((whitespace, block_comment, line_comment))
        .repeated()
        .ignored();

    let escape = just('\\').ignore_then(choice((
        just('"').to('"'),
        just('\\').to('\\'),
        just('/').to('/'),
        just('n').to('\n'),
        just('r').to('\r'),
        just('t').to('\t'),
    )));

    let string = just('"')
        .ignore_then(choice((escape, filter(|c: &char| *c != '"' && *c != '\\'))).repeated())
        .then_ignore(just('"'))
        .map(|chars: Vec<char>| chars.into_iter().collect::<String>())
        .map(Token::String);

    let doc_comment = just("///")
        .ignore_then(filter(|c: &char| *c != '\n').repeated())
        .map(|chars: Vec<char>| chars.into_iter().collect::<String>())
        .map(|line| line.trim().to_string())
        .then_ignore(just('\n').or_not())
        .map(Token::DocComment);

    let bool_token = choice((
        just("true")
            .then_ignore(filter(|c: &char| c.is_alphanumeric() || *c == '_').not())
            .to(Token::Boolean(true)),
        just("false")
            .then_ignore(filter(|c: &char| c.is_alphanumeric() || *c == '_').not())
            .to(Token::Boolean(false)),
    ));

    let ident = text::ident().map(Token::Ident);
    let number = text::int(10)
        .then(just('.').then(text::digits(10)).or_not())
        .map(|(mut int_part, fractional)| {
            if let Some((_, frac)) = fractional {
                int_part.push('.');
                int_part.push_str(&frac);
            }
            int_part
        })
        .map(Token::Number);

    let token = choice((
        doc_comment,
        bool_token,
        string,
        number,
        just("@@").to(Token::AtAt),
        just('@').to(Token::At),
        just('{').to(Token::LBrace),
        just('}').to(Token::RBrace),
        just('(').to(Token::LParen),
        just(')').to(Token::RParen),
        just('[').to(Token::LBracket),
        just(']').to(Token::RBracket),
        just(',').to(Token::Comma),
        just(':').to(Token::Colon),
        just('=').to(Token::Equals),
        just('.').to(Token::Dot),
        just('?').to(Token::Question),
        ident,
    ))
    .map_with_span(|token, span| (token, span))
    .padded_by(skip.clone());

    skip.ignore_then(token.repeated())
        .then_ignore(skip)
        .then_ignore(end())
}

fn schema_parser() -> impl Parser<Token, Schema, Error = Simple<Token>> {
    recursive(|_schema| {
        let doc = doc_parser();
        let ident = identifier();

        let value = recursive(|value| {
            let path = ident
                .clone()
                .then(just(Token::Dot).ignore_then(ident.clone()).repeated())
                .map(|(head, tail)| {
                    let mut parts = vec![head];
                    parts.extend(tail);
                    parts
                })
                .boxed();

            let argument = ident
                .clone()
                .then_ignore(just(Token::Colon))
                .then(value.clone())
                .map(|(name, value)| Argument {
                    name: Some(name),
                    value,
                })
                .or(value.clone().map(|value| Argument { name: None, value }))
                .boxed();

            let arguments = argument
                .clone()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .boxed();

            let call_or_path = path
                .clone()
                .then(arguments.clone().or_not())
                .map(|(path, maybe_args)| match maybe_args {
                    Some(arguments) => Value::Function(FunctionCall { path, arguments }),
                    None => Value::Path(path),
                })
                .boxed();

            let array = value
                .clone()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .delimited_by(just(Token::LBracket), just(Token::RBracket))
                .map(Value::Array)
                .boxed();

            choice((
                select! { Token::String(value) => Value::String(value) },
                select! { Token::Number(value) => Value::Number(value) },
                select! { Token::Boolean(value) => Value::Boolean(value) },
                array,
                call_or_path,
            ))
        })
        .boxed();

        let argument = ident
            .clone()
            .then_ignore(just(Token::Colon))
            .then(value.clone())
            .map(|(name, value)| Argument {
                name: Some(name),
                value,
            })
            .or(value.clone().map(|value| Argument { name: None, value }))
            .boxed();

        let arguments = argument
            .clone()
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .boxed();

        let dotted_ident = ident
            .clone()
            .then(just(Token::Dot).ignore_then(ident.clone()).repeated())
            .map(|(head, tail)| {
                if tail.is_empty() {
                    head
                } else {
                    let mut name = head.to_string();
                    for segment in tail {
                        name.push('.');
                        name.push_str(segment.as_str());
                    }
                    Identifier::from(name)
                }
            })
            .boxed();

        let attribute = dotted_ident
            .then(arguments.clone().or_not())
            .map(|(name, args)| Attribute {
                name,
                arguments: args.unwrap_or_default(),
            })
            .boxed();

        let field_attribute = just(Token::At)
            .ignore_then(attribute.clone())
            .map(convert_field_attribute)
            .boxed();

        let block_attribute = just(Token::AtAt)
            .ignore_then(attribute.clone())
            .map(convert_block_attribute)
            .boxed();

        let unsupported_type = select! { Token::Ident(name) if name == "Unsupported" => name }
            .ignore_then(just(Token::LParen))
            .ignore_then(select! { Token::String(value) => value })
            .then_ignore(just(Token::RParen))
            .map(|inner| format!("Unsupported({})", quote_string(&inner)))
            .boxed();

        let named_type = ident
            .clone()
            .then(just(Token::Dot).ignore_then(ident.clone()).repeated())
            .map(|(head, tail)| {
                let mut name = head.to_string();
                for segment in tail {
                    name.push('.');
                    name.push_str(segment.as_str());
                }
                name
            })
            .boxed();

        let type_parser = unsupported_type
            .or(named_type)
            .then(
                just(Token::LBracket)
                    .ignore_then(just(Token::RBracket))
                    .or_not(),
            )
            .then(just(Token::Question).or_not())
            .map(|((name, list), optional)| Type {
                name,
                list: list.is_some(),
                optional: optional.is_some(),
            })
            .boxed();

        let field = doc
            .clone()
            .then(ident.clone())
            .then(type_parser)
            .then(field_attribute.clone().repeated())
            .map(|(((doc, name), r#type), attributes)| Field {
                name,
                r#type,
                attributes,
                documentation: join_doc(doc),
            })
            .boxed();

        let field_or_attr = field
            .clone()
            .map(Either::Left)
            .or(block_attribute.clone().map(Either::Right))
            .boxed();

        let fields_and_attrs = field_or_attr
            .clone()
            .repeated()
            .delimited_by(just(Token::LBrace), just(Token::RBrace))
            .boxed();

        let model = doc
            .clone()
            .then_ignore(keyword("model"))
            .then(ident.clone())
            .then(fields_and_attrs.clone())
            .map(|((doc, name), items)| {
                let (fields, attributes) = split_fields(items);
                Model {
                    name,
                    fields,
                    attributes,
                    documentation: join_doc(doc),
                }
            })
            .boxed();

        let view = doc
            .clone()
            .then_ignore(keyword("view"))
            .then(ident.clone())
            .then(fields_and_attrs.clone())
            .map(|((doc, name), items)| {
                let (fields, attributes) = split_fields(items);
                View {
                    name,
                    fields,
                    attributes,
                    documentation: join_doc(doc),
                }
            })
            .boxed();

        let composite_type = doc
            .clone()
            .then_ignore(keyword("type"))
            .then(ident.clone())
            .then(
                field
                    .clone()
                    .repeated()
                    .delimited_by(just(Token::LBrace), just(Token::RBrace)),
            )
            .map(|((doc, name), fields)| CompositeType {
                name,
                fields,
                documentation: join_doc(doc),
            })
            .boxed();

        let enum_value = doc
            .clone()
            .then(ident.clone())
            .then(field_attribute.clone().repeated())
            .map(|((doc, name), attrs)| {
                let mapped_name = attrs.iter().find_map(|attr| match attr {
                    FieldAttribute::Map(value) => Some(value.clone()),
                    _ => None,
                });
                EnumValue {
                    name,
                    mapped_name,
                    documentation: join_doc(doc),
                }
            })
            .boxed();

        let enum_items = enum_value
            .clone()
            .map(Either::Left)
            .or(block_attribute.clone().map(Either::Right))
            .repeated()
            .delimited_by(just(Token::LBrace), just(Token::RBrace))
            .boxed();

        let enum_block = doc
            .clone()
            .then_ignore(keyword("enum"))
            .then(ident.clone())
            .then(enum_items)
            .map(|((doc, name), items)| {
                let (values, attributes) = split_enum(items);
                Enum {
                    name,
                    values,
                    attributes,
                    documentation: join_doc(doc),
                }
            })
            .boxed();

        let config_property = doc
            .clone()
            .then(ident.clone())
            .then(
                select! { Token::Equals => () }
                    .ignore_then(value.clone())
                    .or_not(),
            )
            .map(|((doc, name), value)| ConfigProperty {
                name,
                value: value.map(|value| value.to_string()),
                documentation: join_doc(doc),
            })
            .boxed();

        let config_block = move |kind: ConfigBlockKind, keyword_name: &'static str| {
            doc.clone()
                .then_ignore(keyword(keyword_name))
                .then(ident.clone())
                .then(
                    config_property
                        .clone()
                        .repeated()
                        .delimited_by(just(Token::LBrace), just(Token::RBrace)),
                )
                .map(move |((doc_lines, name), properties)| ConfigBlock {
                    kind,
                    name,
                    properties,
                    documentation: join_doc(doc_lines),
                })
                .boxed()
        };

        let top = choice((
            model.map(Top::Model),
            view.map(Top::View),
            composite_type.map(Top::CompositeType),
            enum_block.map(Top::Enum),
            config_block(ConfigBlockKind::Datasource, "datasource").map(Top::Datasource),
            config_block(ConfigBlockKind::Generator, "generator").map(Top::Generator),
        ));

        top.repeated().then_ignore(end()).map(|items| {
            let mut schema = Schema::default();
            for item in items {
                match item {
                    Top::Model(model) => schema.models.push(model),
                    Top::View(view) => schema.views.push(view),
                    Top::CompositeType(ct) => schema.composite_types.push(ct),
                    Top::Enum(enm) => schema.enums.push(enm),
                    Top::Datasource(ds) => schema.datasources.push(ds),
                    Top::Generator(generator) => schema.generators.push(generator),
                }
            }
            schema
        })
    })
}

#[derive(Clone, Debug)]
enum Either<L, R> {
    Left(L),
    Right(R),
}

#[derive(Debug)]
enum Top {
    Model(Model),
    View(View),
    CompositeType(CompositeType),
    Enum(Enum),
    Datasource(ConfigBlock),
    Generator(ConfigBlock),
}

fn doc_parser() -> BoxedParser<'static, Token, Vec<String>, Simple<Token>> {
    select! { Token::DocComment(doc) => doc }.repeated().boxed()
}

fn identifier() -> BoxedParser<'static, Token, Identifier, Simple<Token>> {
    select! { Token::Ident(ident) => Identifier::from(ident) }.boxed()
}

fn keyword(expected: &'static str) -> impl Parser<Token, (), Error = Simple<Token>> + Clone {
    select! { Token::Ident(ident) if ident == expected => () }
}

fn split_fields(items: Vec<Either<Field, BlockAttribute>>) -> (Vec<Field>, Vec<BlockAttribute>) {
    let mut fields = Vec::new();
    let mut attrs = Vec::new();
    for item in items {
        match item {
            Either::Left(field) => fields.push(field),
            Either::Right(attr) => attrs.push(attr),
        }
    }
    (fields, attrs)
}

fn split_enum(
    items: Vec<Either<EnumValue, BlockAttribute>>,
) -> (Vec<EnumValue>, Vec<BlockAttribute>) {
    let mut values = Vec::new();
    let mut attrs = Vec::new();
    for item in items {
        match item {
            Either::Left(value) => values.push(value),
            Either::Right(attr) => attrs.push(attr),
        }
    }
    (values, attrs)
}

fn join_doc(doc: Vec<String>) -> Option<String> {
    if doc.is_empty() {
        None
    } else {
        Some(doc.join("\n"))
    }
}

fn convert_field_attribute(attr: Attribute) -> FieldAttribute {
    let name = attr.name.as_str();
    match name {
        "id" => FieldAttribute::Id,
        "unique" => FieldAttribute::Unique,
        "default" => attr
            .arguments
            .first()
            .map(|arg| FieldAttribute::Default(convert_default_value(&arg.value)))
            .unwrap_or_else(|| FieldAttribute::Raw(format_attribute("@", &attr))),
        "map" => attr
            .arguments
            .first()
            .and_then(|arg| match &arg.value {
                Value::String(value) => Some(FieldAttribute::Map(value.clone())),
                Value::Path(path) if path.len() == 1 => {
                    Some(FieldAttribute::Map(path[0].to_string()))
                }
                _ => None,
            })
            .unwrap_or_else(|| FieldAttribute::Raw(format_attribute("@", &attr))),
        "relation" => convert_relation_attribute(&attr)
            .map(FieldAttribute::Relation)
            .unwrap_or_else(|| FieldAttribute::Raw(format_attribute("@", &attr))),
        name if name.starts_with("db.") => FieldAttribute::DbNative(format_attribute("@", &attr)),
        _ => FieldAttribute::Raw(format_attribute("@", &attr)),
    }
}

fn convert_block_attribute(attr: Attribute) -> BlockAttribute {
    let name = attr.name.as_str();
    match name {
        "id" => BlockAttribute::Id(extract_ident_list(&attr)),
        "unique" => BlockAttribute::Unique(extract_ident_list(&attr)),
        "index" => BlockAttribute::Index(extract_ident_list(&attr)),
        "map" => attr
            .arguments
            .first()
            .and_then(|arg| match &arg.value {
                Value::String(value) => Some(BlockAttribute::Map(value.clone())),
                Value::Path(path) if path.len() == 1 => {
                    Some(BlockAttribute::Map(path[0].to_string()))
                }
                _ => None,
            })
            .unwrap_or_else(|| BlockAttribute::Raw(format_attribute("@@", &attr))),
        _ => BlockAttribute::Raw(format_attribute("@@", &attr)),
    }
}

fn convert_default_value(value: &Value) -> DefaultValue {
    match value {
        Value::Function(FunctionCall { path, arguments }) if path.len() == 1 => {
            let name = path[0].as_str();
            match name {
                "now" => DefaultValue::Now,
                "uuid" => DefaultValue::Uuid,
                "autoincrement" => DefaultValue::AutoIncrement,
                "dbgenerated" => arguments
                    .first()
                    .map(|arg| match &arg.value {
                        Value::String(s) => DefaultValue::DbGenerated(s.clone()),
                        _ => DefaultValue::DbGenerated(arg.value.to_string()),
                    })
                    .unwrap_or_else(|| DefaultValue::Expression(value.to_string())),
                _ => DefaultValue::Expression(value.to_string()),
            }
        }
        _ => DefaultValue::Expression(value.to_string()),
    }
}

fn convert_relation_attribute(attr: &Attribute) -> Option<RelationAttribute> {
    let mut name = None;
    let mut fields = Vec::new();
    let mut references = Vec::new();
    let mut map = None;
    let mut on_delete = None;
    let mut on_update = None;

    for argument in &attr.arguments {
        match argument.name.as_ref().map(|name| name.as_str()) {
            Some("fields") => fields = extract_identifier_array(&argument.value),
            Some("references") => references = extract_identifier_array(&argument.value),
            Some("name") => name = Some(extract_string_like(&argument.value)),
            Some("map") => map = Some(extract_string_like(&argument.value)),
            Some("onDelete") => on_delete = Some(argument.value.to_string()),
            Some("onUpdate") => on_update = Some(argument.value.to_string()),
            _ => {
                if fields.is_empty() {
                    fields = extract_identifier_array(&argument.value);
                } else if references.is_empty() {
                    references = extract_identifier_array(&argument.value);
                } else if name.is_none() {
                    name = Some(extract_string_like(&argument.value));
                } else if map.is_none() {
                    map = Some(extract_string_like(&argument.value));
                }
            }
        }
    }

    Some(RelationAttribute {
        name,
        fields,
        references,
        map,
        on_delete,
        on_update,
    })
}

fn extract_ident_list(attr: &Attribute) -> Vec<Identifier> {
    attr.arguments
        .iter()
        .find_map(|arg| match &arg.value {
            Value::Array(values) => Some(
                values
                    .iter()
                    .filter_map(|value| match value {
                        Value::Path(path) if path.len() == 1 => Some(path[0]),
                        Value::String(value) => Some(Identifier::from(value.clone())),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}

fn extract_identifier_array(value: &Value) -> Vec<Identifier> {
    match value {
        Value::Array(values) => values
            .iter()
            .filter_map(|value| match value {
                Value::Path(path) if path.len() == 1 => Some(path[0]),
                Value::String(value) => Some(Identifier::from(value.clone())),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn extract_string_like(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Path(path) if path.len() == 1 => path[0].to_string(),
        _ => value.to_string(),
    }
}

fn format_attribute(prefix: &str, attr: &Attribute) -> String {
    let args = if attr.arguments.is_empty() {
        String::new()
    } else {
        let inner = attr
            .arguments
            .iter()
            .map(|arg| arg.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!("({})", inner)
    };
    format!("{}{}{}", prefix, attr.name, args)
}

fn render_errors<T>(errors: Vec<Simple<T>>, source: &str) -> anyhow::Error
where
    T: fmt::Display + std::cmp::Eq + std::hash::Hash,
{
    let mut rendered = String::new();
    for error in errors {
        let report = Report::build(ReportKind::Error, (), error.span().start)
            .with_message(error.to_string())
            .with_label(
                Label::new(error.span())
                    .with_message(match error.reason() {
                        SimpleReason::Unexpected => {
                            let expected = error
                                .expected()
                                .filter_map(|expected| {
                                    expected.as_ref().map(|value| value.to_string())
                                })
                                .collect::<Vec<_>>();
                            if expected.is_empty() {
                                "unexpected token".to_string()
                            } else {
                                format!("expected {}", expected.join(", ").fg(Color::Green))
                            }
                        }
                        SimpleReason::Unclosed { delimiter, .. } => {
                            format!("unclosed delimiter {}", delimiter.fg(Color::Yellow))
                        }
                        SimpleReason::Custom(message) => message.clone(),
                    })
                    .with_color(Color::Red),
            )
            .finish();

        let mut buf = Vec::new();
        report
            .write(Source::from(source), &mut buf)
            .expect("failed to render report");
        rendered.push_str(&String::from_utf8_lossy(&buf));
        rendered.push('\n');
    }
    anyhow!(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_serializes_prisma_schema() {
        let schema_src = r#"
/// Main datasource used for the application
datasource db {
  provider = "postgresql"
  url = env("DATABASE_URL")
}

generator client {
  provider = "prisma-client-js"
}

enum Role {
  USER
  ADMIN @map("admin_role")
}

/// Mailing address information
type Address {
  street String
}

model User {
  id Int @id @default(autoincrement())
  name String? @map("full_name")
  role Role @default(USER)
  posts Post[]

  @@map("users")
}

model Post {
  id Int @id
  userId Int
  user User @relation(fields: [userId], references: [id], map: "post_user", onDelete: Cascade)
}
"#;

        let (tokens, lex_errors) = lexer().parse_recovery(schema_src);
        eprintln!("lex errors: {:?}", lex_errors);
        eprintln!("tokens: {:?}", tokens.as_ref().map(|items| items.len()));
        let schema = parse_schema_str(schema_src).expect("schema parses");
        assert_eq!(schema.datasources.len(), 1);
        assert_eq!(schema.datasources[0].kind, ConfigBlockKind::Datasource);
        assert_eq!(
            schema.datasources[0].documentation.as_deref(),
            Some("Main datasource used for the application")
        );
        assert_eq!(
            schema.datasources[0].properties[0].value.as_deref(),
            Some("\"postgresql\"")
        );
        assert_eq!(
            schema.datasources[0].properties[1].value.as_deref(),
            Some("env(\"DATABASE_URL\")")
        );

        assert_eq!(schema.generators.len(), 1);
        assert_eq!(schema.enums.len(), 1);
        let role_enum = &schema.enums[0];
        assert_eq!(role_enum.name.as_str(), "Role");
        assert_eq!(role_enum.values.len(), 2);
        assert_eq!(
            role_enum.values[1].mapped_name.as_deref(),
            Some("admin_role")
        );

        assert_eq!(schema.composite_types.len(), 1);
        assert_eq!(
            schema.composite_types[0].documentation.as_deref().unwrap(),
            "Mailing address information"
        );

        assert_eq!(schema.models.len(), 2);
        let user = &schema.models[0];
        assert_eq!(user.name.as_str(), "User");
        let id_field = &user.fields[0];
        assert!(matches!(
            id_field.attributes.first(),
            Some(FieldAttribute::Id)
        ));
        let default_attr = id_field
            .attributes
            .iter()
            .find_map(|attr| match attr {
                FieldAttribute::Default(value) => Some(value),
                _ => None,
            })
            .expect("default attribute");
        assert!(matches!(default_attr, DefaultValue::AutoIncrement));

        let name_field = &user.fields[1];
        let mapped = name_field
            .attributes
            .iter()
            .find_map(|attr| match attr {
                FieldAttribute::Map(value) => Some(value),
                _ => None,
            })
            .expect("map attribute");
        assert_eq!(mapped, "full_name");

        let post_model = &schema.models[1];
        let relation_field = post_model
            .fields
            .iter()
            .find(|field| field.name.as_str() == "user")
            .expect("relation field");
        let relation = relation_field
            .attributes
            .iter()
            .find_map(|attr| match attr {
                FieldAttribute::Relation(rel) => Some(rel),
                _ => None,
            })
            .expect("relation attribute");
        assert_eq!(relation.fields.len(), 1);
        assert_eq!(relation.fields[0].as_str(), "userId");
        assert_eq!(relation.map.as_deref(), Some("post_user"));

        let rendered = schema.to_string();
        let reparsed = parse_schema_str(&rendered).expect("rendered schema parses");
        assert_eq!(reparsed.models.len(), schema.models.len());
        assert_eq!(reparsed.enums.len(), schema.enums.len());
        assert_eq!(reparsed.datasources.len(), schema.datasources.len());
    }

    #[test]
    fn parses_complex_model_attributes() {
        let schema_src = r#"
model Example {
  /// Primary identifier
  /// spanning multiple lines
  id String @id @default(uuid()) @map("id_col")

  value Unsupported("Text")? @db.VarChar(255)
  tags String[] @default(["foo", "bar"])
  otherId Int?
  other RelationModel? @relation(name: "ExampleToRelation", fields: [otherId], references: [id], map: "RelMap", onDelete: SetNull, onUpdate: Cascade)

  @@id([id, otherId])
  @@unique([value])
  @@index([otherId])
  @@map("example_table")
  @@ignore
}

model RelationModel {
  id Int @id
  examples Example[]
}
"#;

        let schema = parse_schema_str(schema_src).expect("schema parses");
        assert_eq!(schema.models.len(), 2, "schema: {schema:?}");
        let example = schema
            .models
            .iter()
            .find(|model| model.name.as_str() == "Example")
            .expect("example model");
        assert_eq!(
            example.fields[0].documentation.as_deref(),
            Some("Primary identifier\nspanning multiple lines")
        );

        let id_attrs = &example.fields[0].attributes;
        assert!(
            id_attrs
                .iter()
                .any(|attr| matches!(attr, FieldAttribute::Id))
        );
        assert!(
            id_attrs
                .iter()
                .any(|attr| matches!(attr, FieldAttribute::Default(DefaultValue::Uuid)))
        );
        assert_eq!(
            id_attrs.iter().find_map(|attr| match attr {
                FieldAttribute::Map(value) => Some(value.as_str()),
                _ => None,
            }),
            Some("id_col")
        );

        let value_field = example
            .fields
            .iter()
            .find(|field| field.name.as_str() == "value")
            .expect("value field");
        assert!(matches!(
            value_field
                .attributes
                .iter()
                .find(|attr| matches!(attr, FieldAttribute::DbNative(_))),
            Some(FieldAttribute::DbNative(db_attr)) if db_attr == "@db.VarChar(255)"
        ));
        assert!(value_field.r#type.optional);

        let tags_field = example
            .fields
            .iter()
            .find(|field| field.name.as_str() == "tags")
            .expect("tags field");
        assert!(tags_field.r#type.list);
        assert!(matches!(
            tags_field
                .attributes
                .iter()
                .find_map(|attr| match attr {
                    FieldAttribute::Default(DefaultValue::Expression(expr)) => Some(expr),
                    _ => None,
                }),
            Some(expr) if expr == "[\"foo\", \"bar\"]"
        ));

        let relation_field = example
            .fields
            .iter()
            .find(|field| field.name.as_str() == "other")
            .expect("relation field");
        let relation = relation_field
            .attributes
            .iter()
            .find_map(|attr| match attr {
                FieldAttribute::Relation(rel) => Some(rel),
                _ => None,
            })
            .expect("relation attribute");
        assert_eq!(relation.name.as_deref(), Some("ExampleToRelation"));
        assert_eq!(
            relation
                .fields
                .iter()
                .map(Identifier::as_str)
                .collect::<Vec<_>>(),
            vec!["otherId"]
        );
        assert_eq!(
            relation
                .references
                .iter()
                .map(Identifier::as_str)
                .collect::<Vec<_>>(),
            vec!["id"]
        );
        assert_eq!(relation.map.as_deref(), Some("RelMap"));
        assert_eq!(relation.on_delete.as_deref(), Some("SetNull"));
        assert_eq!(relation.on_update.as_deref(), Some("Cascade"));

        assert!(
            example
                .attributes
                .iter()
                .any(|attr| matches!(attr, BlockAttribute::Id(columns) if columns.len() == 2))
        );
        assert!(
            example
                .attributes
                .iter()
                .any(|attr| matches!(attr, BlockAttribute::Unique(columns) if columns.len() == 1))
        );
        assert!(
            example
                .attributes
                .iter()
                .any(|attr| matches!(attr, BlockAttribute::Index(columns) if columns.len() == 1))
        );
        assert!(
            example
                .attributes
                .iter()
                .any(|attr| matches!(attr, BlockAttribute::Map(value) if value == "example_table"))
        );
        assert!(
            example
                .attributes
                .iter()
                .any(|attr| matches!(attr, BlockAttribute::Raw(value) if value == "@@ignore"))
        );
    }

    #[test]
    fn parses_config_blocks_and_views() {
        let schema_src = r#"
/// Global generator configuration
/// with multiple lines of docs
generator client {
  /// Binary targets for generated clients
  binaryTargets = ["native", "linux-musl"]
  provider = "prisma-client-js"
  previewFeatures = ["clientExtensions"]
}

datasource db {
  /// Primary provider used for the database
  provider = "postgresql"
  url = env("DATABASE_URL")
  shadowDatabaseUrl = env("SHADOW_DATABASE_URL")
  relationMode = "prisma"
}

view AuditLog {
  id Int @id
  createdAt DateTime @default(now())
}
"#;

        let schema = parse_schema_str(schema_src).expect("schema parses");
        assert_eq!(schema.generators.len(), 1);
        let generator = &schema.generators[0];
        assert_eq!(generator.kind, ConfigBlockKind::Generator);
        assert_eq!(
            generator.documentation.as_deref(),
            Some("Global generator configuration\nwith multiple lines of docs")
        );
        assert_eq!(generator.properties.len(), 3);
        assert_eq!(generator.properties[0].name.as_str(), "binaryTargets");
        assert_eq!(
            generator.properties[0].documentation.as_deref(),
            Some("Binary targets for generated clients")
        );
        assert_eq!(
            generator.properties[0].value.as_deref(),
            Some("[\"native\", \"linux-musl\"]")
        );
        assert_eq!(
            generator
                .properties
                .iter()
                .find(|prop| prop.name.as_str() == "provider")
                .and_then(|prop| prop.value.as_deref()),
            Some("\"prisma-client-js\"")
        );

        assert_eq!(schema.datasources.len(), 1);
        let datasource = &schema.datasources[0];
        assert_eq!(datasource.kind, ConfigBlockKind::Datasource);
        assert_eq!(datasource.name.as_str(), "db");
        assert_eq!(datasource.properties.len(), 4);
        assert_eq!(
            datasource.properties[0].documentation.as_deref(),
            Some("Primary provider used for the database")
        );
        assert_eq!(
            datasource
                .properties
                .iter()
                .find(|prop| prop.name.as_str() == "url")
                .and_then(|prop| prop.value.as_deref()),
            Some("env(\"DATABASE_URL\")")
        );
        assert_eq!(
            datasource
                .properties
                .iter()
                .find(|prop| prop.name.as_str() == "shadowDatabaseUrl")
                .and_then(|prop| prop.value.as_deref()),
            Some("env(\"SHADOW_DATABASE_URL\")")
        );

        assert_eq!(schema.views.len(), 1);
        let view = &schema.views[0];
        assert_eq!(view.name.as_str(), "AuditLog");
        assert_eq!(view.fields.len(), 2);
        assert!(matches!(
            view.fields
                .iter()
                .find(|field| field.name.as_str() == "createdAt")
                .and_then(|field| field.attributes.iter().find_map(|attr| match attr {
                    FieldAttribute::Default(DefaultValue::Now) => Some(()),
                    _ => None,
                })),
            Some(())
        ));
    }
}
