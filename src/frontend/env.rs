use anyhow::{Result, bail};
use hcl::Value;
use std::collections::{BTreeMap, HashMap};

use super::ast::VarValidation;

/// Variables available during expression evaluation.
///
/// The evaluation engine exposes several namespaces:
/// - `var.<name>` for values in [`Self::vars`]
/// - `local.<name>` or `locals.<name>` for values in [`Self::locals`]
/// - `module.<name>.<output>` for outputs produced by modules
/// - `data.<type>.<name>` for values loaded from data sources
/// - `each.key`/`each.value` inside `for_each` blocks
/// - `count.index` inside blocks using the `count` attribute
///
/// # Example
/// ```
/// use dbschema::frontend::env::EnvVars;
/// use hcl::Value;
/// use std::collections::HashMap;
///
/// let env = EnvVars {
///     vars: HashMap::from([( "name".into(), Value::from("world"))]),
///     locals: HashMap::from([( "name".into(), Value::from("bob"))]),
///     modules: HashMap::new(),
///     data: HashMap::new(),
///     each: None,
///     count: None,
/// };
/// // `local.name` resolves to "bob" while `var.name` resolves to "world".
/// ```
#[derive(Default, Clone, Debug)]
pub struct EnvVars {
    /// Variables passed from the outside world, resolved as `var.*`.
    pub vars: HashMap<String, Value>,
    /// Locally defined values, resolved as `local.*` or `locals.*`.
    pub locals: HashMap<String, Value>,
    /// Outputs from loaded modules, referenced as `module.<name>.<output>`.
    pub modules: HashMap<String, HashMap<String, Value>>,
    /// Values loaded via `data` blocks, referenced as `data.<type>.<name>`.
    pub data: HashMap<String, HashMap<String, Value>>,
    /// Key/value for the current iteration of a `for_each` block, enabling `each.key` and `each.value`.
    pub each: Option<(Value, Value)>, // (key, value)
    /// Index for `count`-based iterations, enabling `count.index`.
    pub count: Option<usize>,
}

#[derive(Clone, Debug)]
pub enum VarType {
    String,
    Number,
    Bool,
    List(Box<VarType>),
    Map(Box<VarType>),
    Object(BTreeMap<String, ObjectField>),
}

#[derive(Clone, Debug)]
pub struct ObjectField {
    pub r#type: VarType,
    pub optional: bool,
}

impl std::fmt::Display for VarType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VarType::String => write!(f, "string"),
            VarType::Number => write!(f, "number"),
            VarType::Bool => write!(f, "bool"),
            VarType::List(inner) => write!(f, "list({inner})"),
            VarType::Map(inner) => write!(f, "map({inner})"),
            VarType::Object(fields) => {
                let parts = fields
                    .iter()
                    .map(|(k, field)| {
                        let key = format_field_key(k);
                        let ty = if field.optional {
                            format!("optional({})", field.r#type)
                        } else {
                            field.r#type.to_string()
                        };
                        format!("{key} = {ty}")
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "object({{{parts}}})")
            }
        }
    }
}

impl std::str::FromStr for VarType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut parser = TypeParser::new(s);
        let ty = parser.parse_type()?;
        parser.skip_ws();
        if parser.peek().is_some() {
            bail!("unexpected trailing characters in type '{}'", s);
        }
        Ok(ty)
    }
}

#[derive(Default, Clone, Debug)]
pub struct VarSpec {
    pub default: Option<Value>,
    pub r#type: Option<VarType>,
    pub validation: Option<VarValidation>,
}

struct TypeParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> TypeParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn expect(&mut self, expected: char) -> Result<()> {
        self.skip_ws();
        match self.advance() {
            Some(c) if c == expected => Ok(()),
            Some(c) => bail!("expected '{expected}', found '{c}'"),
            None => bail!("expected '{expected}', found end of input"),
        }
    }

    fn parse_identifier(&mut self) -> Result<String> {
        self.skip_ws();
        let mut ident = String::new();
        match self.peek() {
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                ident.push(c);
                self.advance();
            }
            Some(c) => bail!("unexpected character '{c}' in type"),
            None => bail!("unexpected end of input"),
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '_' {
                ident.push(c);
                self.advance();
            } else {
                break;
            }
        }
        Ok(ident)
    }

    fn parse_type(&mut self) -> Result<VarType> {
        self.skip_ws();
        let ident = self.parse_identifier()?;
        match ident.as_str() {
            "string" => Ok(VarType::String),
            "number" => Ok(VarType::Number),
            "bool" | "boolean" => Ok(VarType::Bool),
            "list" | "array" => {
                self.expect('(')?;
                let inner = self.parse_type()?;
                self.expect(')')?;
                Ok(VarType::List(Box::new(inner)))
            }
            "map" => {
                self.expect('(')?;
                let inner = self.parse_type()?;
                self.expect(')')?;
                Ok(VarType::Map(Box::new(inner)))
            }
            "object" => {
                self.expect('(')?;
                self.skip_ws();
                self.expect('{')?;
                let mut fields = BTreeMap::new();
                loop {
                    self.skip_ws();
                    if let Some('}') = self.peek() {
                        self.advance();
                        break;
                    }
                    let field_name = self.parse_field_name()?;
                    self.skip_ws();
                    self.expect('=')?;
                    let (field_type, optional) = self.parse_object_field_type()?;
                    if fields
                        .insert(
                            field_name.clone(),
                            ObjectField {
                                r#type: field_type,
                                optional,
                            },
                        )
                        .is_some()
                    {
                        bail!("duplicate field '{field_name}' in object type");
                    }
                    self.skip_ws();
                    match self.peek() {
                        Some(',') => {
                            self.advance();
                        }
                        Some('}') => continue,
                        _ => {}
                    }
                }
                self.skip_ws();
                self.expect(')')?;
                Ok(VarType::Object(fields))
            }
            _ => bail!("unknown type '{ident}'"),
        }
    }

    fn parse_field_name(&mut self) -> Result<String> {
        self.skip_ws();
        if self.peek() == Some('"') {
            self.parse_string_literal()
        } else {
            self.parse_identifier()
        }
    }

    fn parse_object_field_type(&mut self) -> Result<(VarType, bool)> {
        self.skip_ws();
        if self.lookahead_keyword("optional") {
            self.consume_keyword("optional")?;
            self.expect('(')?;
            let ty = self.parse_type()?;
            self.skip_ws();
            if matches!(self.peek(), Some(',')) {
                bail!("optional(...) with defaults is not supported yet");
            }
            self.expect(')')?;
            Ok((ty, true))
        } else {
            let ty = self.parse_type()?;
            Ok((ty, false))
        }
    }

    fn lookahead_keyword(&self, kw: &str) -> bool {
        let remaining = &self.input[self.pos..];
        if !remaining.starts_with(kw) {
            return false;
        }
        let after = remaining[kw.len()..].chars().next();
        after.map_or(true, |c| c == '(' || c.is_whitespace())
    }

    fn consume_keyword(&mut self, kw: &str) -> Result<()> {
        if !self.lookahead_keyword(kw) {
            bail!("expected keyword '{kw}'");
        }
        for _ in 0..kw.len() {
            self.advance();
        }
        Ok(())
    }

    fn parse_string_literal(&mut self) -> Result<String> {
        // assume current char is '"'
        if self.advance() != Some('"') {
            bail!("expected string literal");
        }
        let mut result = String::new();
        while let Some(ch) = self.advance() {
            match ch {
                '"' => return Ok(result),
                '\\' => {
                    let esc = self
                        .advance()
                        .ok_or_else(|| anyhow::anyhow!("unterminated escape in string literal"))?;
                    let decoded = match esc {
                        '"' => '"',
                        '\\' => '\\',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        other => other,
                    };
                    result.push(decoded);
                }
                other => result.push(other),
            }
        }
        bail!("unterminated string literal");
    }
}

fn format_field_key(key: &str) -> String {
    if key
        .chars()
        .next()
        .map(|c| c.is_ascii_alphabetic() || c == '_')
        .unwrap_or(false)
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        key.to_string()
    } else {
        let escaped = key.replace('"', "\\\"");
        format!("\"{escaped}\"")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_object_type() {
        let ty: VarType = "list(object({ name = string, type = string, nullable = bool }))"
            .parse()
            .unwrap();
        match ty {
            VarType::List(inner) => match *inner {
                VarType::Object(ref fields) => {
                    let name = fields.get("name").unwrap();
                    assert!(matches!(name.r#type, VarType::String));
                    assert!(!name.optional);
                    let ty_field = fields.get("type").unwrap();
                    assert!(matches!(ty_field.r#type, VarType::String));
                    let nullable = fields.get("nullable").unwrap();
                    assert!(matches!(nullable.r#type, VarType::Bool));
                }
                other => panic!("expected object type, got {other}"),
            },
            _ => panic!("expected list type"),
        }
    }

    #[test]
    fn rejects_unknown_type_tokens() {
        let err = "widget".parse::<VarType>().unwrap_err();
        assert!(err.to_string().contains("unknown type 'widget'"));
    }

    #[test]
    fn parses_object_with_quoted_and_optional_fields() {
        let ty: VarType = r#"object({"display-name" = optional(string), data = map(list(number))})"#
            .parse()
            .unwrap();
        match ty {
            VarType::Object(ref fields) => {
                let display = fields.get("display-name").unwrap();
                assert!(display.optional);
                assert!(matches!(display.r#type, VarType::String));
                let data = fields.get("data").unwrap();
                assert!(!data.optional);
                match &data.r#type {
                    VarType::Map(inner) => match &**inner {
                        VarType::List(list_inner) => match &**list_inner {
                            VarType::Number => {}
                            other => panic!("unexpected inner type {:?}", other),
                        },
                        other => panic!("unexpected inner type {:?}", other),
                    },
                    other => panic!("unexpected type {:?}", other),
                }
            }
            other => panic!("expected object type, got {:?}", other),
        }
    }

    #[test]
    fn parses_map_of_lists() {
        let ty: VarType = "map(list(string))".parse().unwrap();
        match &ty {
            VarType::Map(inner) => match &**inner {
                VarType::List(list_inner) => match &**list_inner {
                    VarType::String => {}
                    other => panic!("unexpected inner type {:?}", other),
                },
                other => panic!("unexpected inner type {:?}", other),
            },
            other => panic!("unexpected type {:?}", other),
        }
    }
}
