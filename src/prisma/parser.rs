use std::fmt;

use anyhow::{anyhow, Result};
use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
use chumsky::error::{Rich, RichPattern, RichReason};
use chumsky::extra;
use chumsky::input::{MapExtra, Stream, ValueInput};
use chumsky::prelude::*;
use chumsky::util::MaybeRef;

use super::ast::*;

type Span = std::ops::Range<usize>;
type Spanned<T> = (T, Span);

type LexError<'src> = Rich<'src, char, SimpleSpan<usize>>;
type ParseError<'src> = Rich<'src, Token, Span>;
type LexExtra<'src> = extra::Err<LexError<'src>>;
type ParseExtra<'src> = extra::Err<ParseError<'src>>;

trait ToRange {
    fn to_range(&self) -> std::ops::Range<usize>;
}

impl ToRange for std::ops::Range<usize> {
    fn to_range(&self) -> std::ops::Range<usize> {
        self.clone()
    }
}

impl ToRange for SimpleSpan<usize> {
    fn to_range(&self) -> std::ops::Range<usize> {
        self.clone().into_range()
    }
}

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
    let (tokens, lex_errors) = lexer().parse(input).into_output_errors();
    if !lex_errors.is_empty() {
        return Err(render_errors(lex_errors, input));
    }
    let tokens = tokens.ok_or_else(|| anyhow!("failed to tokenize Prisma schema"))?;

    let len = input.len();
    let stream = Stream::from_iter(tokens.into_iter())
        .map(Span::new((), len..len), |(token, span)| (token, span));

    let (schema, parse_errors) = schema_parser::<_>(input)
        .parse(stream)
        .into_output_errors();
    if !parse_errors.is_empty() {
        return Err(render_errors(parse_errors, input));
    }

    schema.ok_or_else(|| anyhow!("failed to parse Prisma schema"))
}

fn lexer<'src>() -> impl Parser<'src, &'src str, Vec<Spanned<Token>>, extra::Err<LexError<'src>>> {
    let whitespace = any::<_, LexExtra<'src>>()
        .filter(|c: &char| c.is_whitespace())
        .ignored();

    let block_comment = just("/*")
        .ignore_then(
            just("*/")
                .not()
                .ignore_then(any::<_, LexExtra<'src>>().ignored())
                .repeated(),
        )
        .then_ignore(just("*/"))
        .ignored();

    let line_comment = just("//")
        .then(just('/').not())
        .ignore_then(
            any::<_, LexExtra<'src>>()
                .filter(|c: &char| *c != '\n')
                .repeated(),
        )
        .then_ignore(just('\n').or_not())
        .ignored();

    let skip = choice((whitespace, block_comment, line_comment))
        .repeated()
        .ignored();

    let unicode_escape = just('u')
        .ignore_then(
            any::<_, LexExtra<'src>>()
                .filter(|c: &char| c.is_ascii_hexdigit())
                .repeated()
                .exactly(4)
                .collect::<String>(),
        )
        .map(|hex| {
            u32::from_str_radix(&hex, 16)
                .ok()
                .and_then(char::from_u32)
                .unwrap_or('\u{FFFD}')
        });

    let escape = just('\\').ignore_then(choice((
        just('"').to('"'),
        just('\\').to('\\'),
        just('/').to('/'),
        just('n').to('\n'),
        just('r').to('\r'),
        just('t').to('\t'),
        unicode_escape,
    )));

    let string = just('"')
        .ignore_then(
            choice((
                escape.clone(),
                any::<_, LexExtra<'src>>()
                    .filter(|c: &char| *c != '"' && *c != '\\'),
            ))
            .repeated()
            .collect::<String>(),
        )
        .then_ignore(just('"'))
        .map(Token::String);

    let doc_comment = just("///")
        .ignore_then(
            any::<_, LexExtra<'src>>()
                .filter(|c: &char| *c != '\n')
                .repeated()
                .collect::<String>(),
        )
        .map(|line| line.trim().to_string())
        .then_ignore(just('\n').or_not())
        .map(Token::DocComment);

    let ident_char = any::<_, LexExtra<'src>>()
        .filter(|c: &char| c.is_alphanumeric() || *c == '_' || *c == '-');

    let bool_token = choice((
        just("true")
            .then_ignore(ident_char.clone().not())
            .to(Token::Boolean(true)),
        just("false")
            .then_ignore(ident_char.clone().not())
            .to(Token::Boolean(false)),
    ));

    let ident = ident_char
        .clone()
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(Token::Ident);

    let digits = any::<_, LexExtra<'src>>()
        .filter(|c: &char| c.is_ascii_digit())
        .repeated()
        .at_least(1)
        .collect::<String>();

    let number = just('-')
        .or_not()
        .then(digits.clone())
        .then(
            just('.')
                .ignore_then(digits.clone())
                .or_not(),
        )
        .map(|((sign, int_part), fractional)| {
            let mut number = String::new();
            if sign.is_some() {
                number.push('-');
            }
            number.push_str(&int_part);
            if let Some(frac) = fractional {
                number.push('.');
                number.push_str(&frac);
            }
            Token::Number(number)
        });

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
    .map_with(|token, extra: &mut MapExtra<'_, '_, &'src str, LexExtra<'src>>| {
        (token, extra.span().to_range())
    })
    .padded_by(skip.clone());

    skip.ignore_then(token.repeated().collect::<Vec<_>>())
        .then_ignore(skip)
        .then_ignore(end())
}

fn schema_parser<'src, I>(source: &'src str) -> impl Parser<'src, I, Schema, ParseExtra<'src>>
where
    I: Input<'src, Token = Token, Span = Span> + ValueInput<'src, Token = Token, Span = Span>,
{
    recursive(|_schema| {
        let doc = doc_parser::<I>();
        let ident = identifier::<I>();

        let value = recursive(|value| {
            let path = ident
                .clone()
                .then(
                    just(Token::Dot)
                        .ignore_then(ident.clone())
                        .repeated()
                        .collect::<Vec<_>>(),
                )
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
                .collect::<Vec<_>>()
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
                .collect::<Vec<_>>()
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
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .boxed();

        let dotted_ident = ident
            .clone()
            .then(
                just(Token::Dot)
                    .ignore_then(ident.clone())
                    .repeated()
                    .collect::<Vec<_>>(),
            )
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
            .then(
                just(Token::Dot)
                    .ignore_then(ident.clone())
                    .repeated()
                    .collect::<Vec<_>>(),
            )
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
            .then(just(Token::Colon).or_not())
            .then(type_parser.clone().or_not())
            .then(field_attribute.clone().repeated().collect::<Vec<_>>())
            .map(|((((doc, name), _colon), maybe_type), attributes)| {
                let r#type = maybe_type.unwrap_or_else(|| Type {
                    name: "Unsupported".to_string(),
                    optional: false,
                    list: false,
                });
                Field {
                    name,
                    r#type,
                    attributes,
                    documentation: join_doc(doc),
                }
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
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LBrace), just(Token::RBrace))
            .boxed();

        let model = doc
            .clone()
            .then_ignore(keyword::<I>("model"))
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
            .then_ignore(keyword::<I>("view"))
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
            .then_ignore(keyword::<I>("type"))
            .then(ident.clone())
            .then(
                field
                    .clone()
                    .repeated()
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::LBrace), just(Token::RBrace)),
            )
            .map(|((doc, name), fields)| CompositeType {
                name,
                fields,
                documentation: join_doc(doc),
            })
            .boxed();

        let type_alias = doc
            .clone()
            .then_ignore(keyword::<I>("type"))
            .then(ident.clone())
            .then_ignore(just(Token::Equals))
            .then(type_parser.clone().or_not())
            .then(field_attribute.clone().repeated().collect::<Vec<_>>())
            .map(|(((doc, name), target), attributes)| {
                let target = target.unwrap_or_else(|| Type {
                    name: "Unsupported".to_string(),
                    optional: false,
                    list: false,
                });
                TypeAlias {
                    name,
                    target,
                    attributes,
                    documentation: join_doc(doc),
                }
            })
            .boxed();

        let ignored_block_body = recursive(|body| {
            choice((
                just(Token::LBrace)
                    .ignore_then(body.clone())
                    .then_ignore(just(Token::RBrace))
                    .ignored(),
                just(Token::RBrace).not().ignore_then(any::<_, ParseExtra<'src>>()).ignored(),
            ))
            .repeated()
            .ignored()
        });

        let block_span = just(Token::LBrace)
            .map_with(|_, extra: &mut MapExtra<'_, '_, I, ParseExtra<'src>>| extra.span().start)
            .then(ignored_block_body.clone())
            .then(
                just(Token::RBrace)
                    .map_with(|_, extra: &mut MapExtra<'_, '_, I, ParseExtra<'src>>| extra.span().end),
            )
            .map(|((start, ()), end)| start..end)
            .boxed();

        let source_ref = source;

        let arbitrary_block = doc
            .clone()
            .then(ident.clone())
            .then(block_span.clone())
            .map(move |((doc_lines, name), span)| {
                let contents = source_ref[span.start..span.end].to_string();
                CustomBlock {
                    name,
                    contents,
                    documentation: join_doc(doc_lines),
                }
            })
            .boxed();

        let enum_value = doc
            .clone()
            .then(ident.clone())
            .then(field_attribute.clone().repeated().collect::<Vec<_>>())
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
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LBrace), just(Token::RBrace))
            .boxed();

        let enum_block = doc
            .clone()
            .then_ignore(keyword::<I>("enum"))
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
                .then_ignore(keyword::<I>(keyword_name))
                .then(ident.clone())
                .then(
                    config_property
                        .clone()
                        .repeated()
                        .collect::<Vec<_>>()
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
            type_alias.map(Top::TypeAlias),
            arbitrary_block.map(Top::Custom),
        ));

        top.repeated()
            .collect::<Vec<_>>()
            .then_ignore(end())
            .map(|items| {
            let mut schema = Schema::default();
            for item in items {
                match item {
                    Top::Model(model) => schema.models.push(model),
                    Top::View(view) => schema.views.push(view),
                    Top::CompositeType(ct) => schema.composite_types.push(ct),
                    Top::Enum(enm) => schema.enums.push(enm),
                    Top::Datasource(ds) => schema.datasources.push(ds),
                    Top::Generator(generator) => schema.generators.push(generator),
                    Top::TypeAlias(alias) => schema.type_aliases.push(alias),
                    Top::Custom(block) => schema.custom_blocks.push(block),
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
    TypeAlias(TypeAlias),
    Custom(CustomBlock),
}

fn doc_parser<'src, I>() -> impl Parser<'src, I, Vec<String>, ParseExtra<'src>> + Clone
where
    I: Input<'src, Token = Token, Span = Span>,
{
    select! { Token::DocComment(doc) => doc }
        .repeated()
        .collect::<Vec<_>>()
}

fn identifier<'src, I>() -> impl Parser<'src, I, Identifier, ParseExtra<'src>> + Clone
where
    I: Input<'src, Token = Token, Span = Span>,
{
    select! { Token::Ident(ident) => Identifier::from(ident) }
}

fn keyword<'src, I>(expected: &'static str) -> impl Parser<'src, I, (), ParseExtra<'src>> + Clone
where
    I: Input<'src, Token = Token, Span = Span>,
{
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
        "id" if attr.arguments.is_empty() => FieldAttribute::Id,
        "id" => FieldAttribute::Raw(format_attribute("@", &attr)),
        "unique" if attr.arguments.is_empty() => FieldAttribute::Unique,
        "unique" => FieldAttribute::Raw(format_attribute("@", &attr)),
        "default" => attr
            .arguments
            .first()
            .map(|arg| FieldAttribute::Default(convert_default_value(&arg.value)))
            .unwrap_or_else(|| FieldAttribute::Raw(format_attribute("@", &attr))),
        "map" if attr.arguments.len() == 1 => attr
            .arguments
            .first()
            .and_then(|arg| match &arg.value {
                Value::String(value) => Some(FieldAttribute::Map(value.clone())),
                Value::Path(path) => Some(FieldAttribute::Map(path_to_identifier(path).to_string())),
                _ => None,
            })
            .unwrap_or_else(|| FieldAttribute::Raw(format_attribute("@", &attr))),
        "map" => FieldAttribute::Raw(format_attribute("@", &attr)),
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
        "id" if attr.arguments.len() == 1 && attr.arguments[0].name.is_none() => {
            extract_identifier_list(&attr)
                .map(BlockAttribute::Id)
                .unwrap_or_else(|| BlockAttribute::Raw(format_attribute("@@", &attr)))
        }
        "id" => BlockAttribute::Raw(format_attribute("@@", &attr)),
        "unique" if attr.arguments.len() == 1 && attr.arguments[0].name.is_none() => {
            extract_identifier_list(&attr)
                .map(BlockAttribute::Unique)
                .unwrap_or_else(|| BlockAttribute::Raw(format_attribute("@@", &attr)))
        }
        "unique" => BlockAttribute::Raw(format_attribute("@@", &attr)),
        "index" if attr.arguments.len() == 1 && attr.arguments[0].name.is_none() => {
            extract_identifier_list(&attr)
                .map(BlockAttribute::Index)
                .unwrap_or_else(|| BlockAttribute::Raw(format_attribute("@@", &attr)))
        }
        "index" => BlockAttribute::Raw(format_attribute("@@", &attr)),
        "map" if attr.arguments.len() == 1 => attr
            .arguments
            .first()
            .and_then(|arg| match &arg.value {
                Value::String(value) => Some(BlockAttribute::Map(value.clone())),
                Value::Path(path) => Some(BlockAttribute::Map(path_to_identifier(path).to_string())),
                _ => None,
            })
            .unwrap_or_else(|| BlockAttribute::Raw(format_attribute("@@", &attr))),
        "map" => BlockAttribute::Raw(format_attribute("@@", &attr)),
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
                "cuid" => DefaultValue::Expression(value.to_string()),
                "nanoid" => DefaultValue::Expression(value.to_string()),
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
                if name.is_none()
                    && matches!(
                        &argument.value,
                        Value::String(_) | Value::Path(_)
                    )
                {
                    name = Some(extract_string_like(&argument.value));
                } else if fields.is_empty() {
                    fields = extract_identifier_array(&argument.value);
                } else if references.is_empty() {
                    references = extract_identifier_array(&argument.value);
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

fn path_to_identifier(path: &[Identifier]) -> Identifier {
    if path.len() == 1 {
        path[0]
    } else {
        let mut name = String::new();
        for (idx, segment) in path.iter().enumerate() {
            if idx > 0 {
                name.push('.');
            }
            name.push_str(segment.as_str());
        }
        Identifier::from(name)
    }
}

fn extract_identifier_list(attr: &Attribute) -> Option<Vec<Identifier>> {
    if attr.arguments.len() != 1 || attr.arguments[0].name.is_some() {
        return None;
    }
    match &attr.arguments[0].value {
        Value::Array(values) => {
            let mut items = Vec::new();
            for value in values {
                match value {
                    Value::Path(path) => items.push(path_to_identifier(path)),
                    Value::String(value) => items.push(Identifier::from(value.clone())),
                    _ => return None,
                }
            }
            Some(items)
        }
        _ => None,
    }
}

fn extract_identifier_array(value: &Value) -> Vec<Identifier> {
    match value {
        Value::Array(values) => values
            .iter()
            .map(|value| match value {
                Value::Path(path) => path_to_identifier(path),
                Value::String(value) => Identifier::from(value.clone()),
                _ => Identifier::from(value.to_string()),
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

fn render_errors<'a, T, S>(errors: Vec<Rich<'a, T, S>>, source: &str) -> anyhow::Error
where
    T: fmt::Display + fmt::Debug + std::cmp::Eq + std::hash::Hash + Clone,
    S: ToRange + Clone + fmt::Debug,
{
    let mut rendered = String::new();
    for error in errors {
        let span_range = error.span().to_range();
        let label_message = match error.reason() {
            RichReason::ExpectedFound { expected, .. } => {
                let expected = expected
                    .iter()
                    .map(pattern_to_string)
                    .collect::<Vec<_>>();
                if expected.is_empty() {
                    "unexpected token".to_string()
                } else {
                    let expected_text = expected.join(", ");
                    format!("expected {}", expected_text.fg(Color::Green))
                }
            }
            RichReason::Custom(message) => message.clone(),
        };

        let report_message = match error.reason() {
            RichReason::ExpectedFound { found, .. } => {
                let found = found
                    .as_ref()
                    .map(format_found)
                    .unwrap_or_else(|| "end of input".to_string());
                format!("unexpected {}", found)
            }
            RichReason::Custom(message) => message.clone(),
        };

        let report = Report::build(ReportKind::Error, span_range.clone())
            .with_message(report_message)
            .with_label(
                Label::new(span_range)
                    .with_message(label_message)
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

fn pattern_to_string<T: fmt::Display + Clone>(pattern: &RichPattern<'_, T>) -> String {
    match pattern {
        RichPattern::Token(token) => format!("{}", token.clone().into_inner()),
        RichPattern::Label(label) => label.to_string(),
        RichPattern::Identifier(identifier) => identifier.clone(),
        RichPattern::Any => "any token".to_string(),
        RichPattern::SomethingElse => "something else".to_string(),
        RichPattern::EndOfInput => "end of input".to_string(),
    }
}

fn format_found<T: fmt::Display + Clone>(found: &MaybeRef<'_, T>) -> String {
    format!("{}", found.clone().into_inner())
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

        let (tokens, lex_errors) = lexer().parse(schema_src).into_output_errors();
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
    fn parses_model_with_advanced_features() {
        let schema_src = r#"
model Post {
  id          Int           @id @default(autoincrement())
  title       String        @db.VarChar(255)
  slug        String?       @unique(map: "slug_idx")
  rating      Decimal?      @db.Decimal(5, 2)
  metadata    Json?         @default("{}")
  createdAt   DateTime      @default(now())
  updatedAt   DateTime      @updatedAt
  authorId    Int
  author      User          @relation("AuthorPosts", fields: [authorId], references: [id], onDelete: Cascade, onUpdate: NoAction)
  categories  Category[]    @relation(references: [id])
  legacy:     String
  legacyField String?

  @@index([title, createdAt(sort: Desc)], map: "title_created_idx", type: Brin)
  @@unique([authorId, title], map: "author_title_unique")
  @@map("posts")
}

model User {
  id Int @id
}

model Category {
  id Int @id
}
"#;

        let schema = parse_schema_str(schema_src).expect("schema parses");
        let post = schema
            .models
            .iter()
            .find(|model| model.name.as_str() == "Post")
            .expect("post model");

        let title_field = post
            .fields
            .iter()
            .find(|field| field.name.as_str() == "title")
            .expect("title field");
        assert_eq!(title_field.r#type.name, "String");
        assert!(!title_field.r#type.optional);

        let rating_field = post
            .fields
            .iter()
            .find(|field| field.name.as_str() == "rating")
            .expect("rating field");
        assert_eq!(rating_field.r#type.name, "Decimal");
        assert!(rating_field.r#type.optional);

        let relation_field = post
            .fields
            .iter()
            .find(|field| field.name.as_str() == "author")
            .expect("author relation field");
        let relation = relation_field
            .attributes
            .iter()
            .find_map(|attr| match attr {
                FieldAttribute::Relation(rel) => Some(rel),
                _ => None,
            })
            .expect("relation attribute");
        assert_eq!(relation.name.as_deref(), Some("AuthorPosts"));
        assert_eq!(
            relation.fields.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
            vec!["authorId"]
        );
        assert_eq!(
            relation
                .references
                .iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>(),
            vec!["id"]
        );
        assert_eq!(relation.on_delete.as_deref(), Some("Cascade"));
        assert_eq!(relation.on_update.as_deref(), Some("NoAction"));

        let block_attrs = &post.attributes;
        assert!(
            block_attrs
                .iter()
                .any(|attr| matches!(attr, BlockAttribute::Raw(raw) if raw.contains("@@index"))),
            "complex @@index should be preserved as raw attribute"
        );
        assert!(
            block_attrs
                .iter()
                .any(|attr| matches!(attr, BlockAttribute::Map(name) if name == "posts")),
            "@@map should parse to BlockAttribute::Map"
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

    #[test]
    fn parses_type_aliases_and_arbitrary_blocks() {
        let schema_src = r#"
/// Custom ID type
type UserId = Int @map("user_id")

generator client {
  provider = "prisma-client-js"
}

customBlock {
  some random tokens here
  nested {
    still inside
  }
  trailing stuff
}

model Example {
  id UserId @id
}
"#;

        let schema = parse_schema_str(schema_src).expect("schema parses");
        assert_eq!(schema.models.len(), 1);
        assert_eq!(schema.generators.len(), 1);
        assert!(schema.enums.is_empty());
        assert_eq!(schema.type_aliases.len(), 1);
        let alias = &schema.type_aliases[0];
        assert_eq!(alias.name.as_str(), "UserId");
        assert_eq!(alias.target.name, "Int");
        assert!(matches!(
            alias.attributes.first(),
            Some(FieldAttribute::Map(value)) if value == "user_id"
        ));

        assert_eq!(schema.custom_blocks.len(), 1);
        let block = &schema.custom_blocks[0];
        assert_eq!(block.name.as_str(), "customBlock");
        assert!(block.contents.contains("nested"));
        assert!(block.contents.starts_with('{'));

        let rendered = schema.to_string();
        assert!(rendered.contains("type UserId = Int @map(\"user_id\")"));
        assert!(rendered.contains("customBlock {"));

        let reparsed = parse_schema_str(&rendered).expect("reparse custom blocks");
        assert_eq!(reparsed.custom_blocks.len(), 1);
        assert_eq!(reparsed.type_aliases.len(), 1);
    }
}
