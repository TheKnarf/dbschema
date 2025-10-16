use anyhow::{anyhow, bail, Context, Result};
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

pub fn parse_schema_str(input: &str) -> Result<Schema> {
    let cleaned = strip_comments(input);
    let chars: Vec<char> = cleaned.chars().collect();
    let mut idx = 0;
    let mut schema = Schema::default();

    while idx < chars.len() {
        skip_whitespace(&chars, &mut idx);
        if idx >= chars.len() {
            break;
        }

        if matches_keyword(&chars, idx, "model") {
            idx += "model".len();
            skip_whitespace(&chars, &mut idx);
            let name = parse_identifier(&chars, &mut idx).context("model block missing name")?;
            skip_whitespace(&chars, &mut idx);
            if idx >= chars.len() || chars[idx] != '{' {
                bail!("model '{}' missing opening brace", name);
            }
            idx += 1;
            let (body, next_idx) = extract_block(&chars, idx)?;
            idx = next_idx;
            schema.models.push(parse_model_block(name, &body)?);
            continue;
        } else if matches_keyword(&chars, idx, "enum") {
            idx += "enum".len();
            skip_whitespace(&chars, &mut idx);
            let name = parse_identifier(&chars, &mut idx).context("enum block missing name")?;
            skip_whitespace(&chars, &mut idx);
            if idx >= chars.len() || chars[idx] != '{' {
                bail!("enum '{}' missing opening brace", name);
            }
            idx += 1;
            let (body, next_idx) = extract_block(&chars, idx)?;
            idx = next_idx;
            schema.enums.push(parse_enum_block(name, &body)?);
            continue;
        } else if matches_keyword(&chars, idx, "datasource")
            || matches_keyword(&chars, idx, "generator")
        {
            let keyword = if matches_keyword(&chars, idx, "datasource") {
                "datasource"
            } else {
                "generator"
            };
            idx += keyword.len();
            skip_whitespace(&chars, &mut idx);
            // Skip block label
            let _ = parse_identifier(&chars, &mut idx);
            skip_whitespace(&chars, &mut idx);
            if idx < chars.len() && chars[idx] == '{' {
                idx += 1;
                let (_, next_idx) = extract_block(&chars, idx)?;
                idx = next_idx;
                continue;
            }
        }

        // Skip unknown blocks or tokens
        idx += 1;
    }

    Ok(schema)
}

fn parse_model_block(name: String, body: &str) -> Result<Model> {
    let mut fields = Vec::new();
    let mut attributes = Vec::new();
    for stmt in split_statements(body) {
        let trimmed = stmt.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("@@") {
            attributes.push(parse_block_attribute(trimmed));
        } else {
            fields.push(parse_field(trimmed)?);
        }
    }
    Ok(Model {
        name,
        fields,
        attributes,
    })
}

fn parse_enum_block(name: String, body: &str) -> Result<Enum> {
    let mut values = Vec::new();
    let mut attributes = Vec::new();
    for stmt in split_statements(body) {
        let trimmed = stmt.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("@@") {
            attributes.push(parse_block_attribute(trimmed));
        } else {
            values.push(parse_enum_value(trimmed)?);
        }
    }
    Ok(Enum {
        name,
        values,
        attributes,
    })
}

fn parse_field(stmt: &str) -> Result<Field> {
    let trimmed = stmt.trim();
    if trimmed.is_empty() {
        bail!("empty field statement");
    }
    let mut split = trimmed
        .char_indices()
        .find(|(_, ch)| ch.is_whitespace())
        .map(|(i, _)| i)
        .unwrap_or(trimmed.len());
    if split == 0 || split == trimmed.len() {
        bail!("field definition missing type: {trimmed}");
    }
    let name = trimmed[..split].to_string();
    while split < trimmed.len() && trimmed.as_bytes()[split].is_ascii_whitespace() {
        split += 1;
    }
    let rest = &trimmed[split..];
    let (type_part, attrs_part) = split_type_and_attrs(rest);
    let field_type = parse_type(&type_part);
    let attributes = attrs_part
        .map(|s| parse_field_attributes(&s))
        .unwrap_or_default();
    Ok(Field {
        name,
        r#type: field_type,
        attributes,
    })
}

fn parse_enum_value(stmt: &str) -> Result<EnumValue> {
    let trimmed = stmt.trim();
    if trimmed.is_empty() {
        bail!("empty enum value statement");
    }
    let mut end = trimmed.len();
    for (i, ch) in trimmed.char_indices() {
        if ch.is_whitespace() || ch == '@' {
            end = i;
            break;
        }
    }
    let name = trimmed[..end].to_string();
    let rest = trimmed[end..].trim();
    let mut mapped_name = None;
    for attr in split_attributes(rest) {
        if attr.starts_with("@map(") {
            if let Some(value) = parse_string_argument(&attr, "@map(") {
                mapped_name = Some(value);
            }
        }
    }
    Ok(EnumValue { name, mapped_name })
}

fn parse_type(type_str: &str) -> Type {
    let mut name = type_str.trim().to_string();
    let mut optional = false;
    if name.ends_with('?') {
        optional = true;
        name.pop();
        name = name.trim_end().to_string();
    }
    let mut list = false;
    if name.ends_with("[]") {
        list = true;
        name.truncate(name.len().saturating_sub(2));
        name = name.trim_end().to_string();
    }
    Type {
        name: name.trim().to_string(),
        optional,
        list,
    }
}

fn parse_field_attributes(attrs: &str) -> Vec<FieldAttribute> {
    split_attributes(attrs)
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .map(|attr| parse_field_attribute(&attr))
        .collect()
}

fn parse_field_attribute(attr: &str) -> FieldAttribute {
    let trimmed = attr.trim();
    if trimmed.starts_with("@id") {
        FieldAttribute::Id
    } else if trimmed.starts_with("@unique") {
        FieldAttribute::Unique
    } else if trimmed.starts_with("@default(") && trimmed.ends_with(')') {
        let inner = &trimmed["@default(".len()..trimmed.len() - 1];
        FieldAttribute::Default(parse_default_value(inner))
    } else if trimmed.starts_with("@map(") && trimmed.ends_with(')') {
        parse_string_argument(trimmed, "@map(")
            .map(FieldAttribute::Map)
            .unwrap_or_else(|| FieldAttribute::Raw(trimmed.to_string()))
    } else if trimmed.starts_with("@db.") || trimmed.starts_with("@db(") {
        FieldAttribute::DbNative(trimmed.to_string())
    } else if trimmed.starts_with("@relation(") {
        parse_relation_attribute(trimmed)
            .unwrap_or_else(|| FieldAttribute::Raw(trimmed.to_string()))
    } else {
        FieldAttribute::Raw(trimmed.to_string())
    }
}

fn parse_relation_attribute(attr: &str) -> Option<FieldAttribute> {
    let inner = attr.strip_prefix("@relation(")?.strip_suffix(')')?;
    let mut fields = Vec::new();
    let mut references = Vec::new();
    let mut on_delete = None;
    let mut on_update = None;
    for part in split_arguments(inner) {
        let mut pieces = part.splitn(2, ':');
        let key = pieces.next()?.trim();
        let value = pieces.next().map(|v| v.trim()).unwrap_or("");
        match key {
            "fields" => fields = parse_identifier_list(value),
            "references" => references = parse_identifier_list(value),
            "onDelete" => on_delete = Some(value.trim_matches('"').to_string()),
            "onUpdate" => on_update = Some(value.trim_matches('"').to_string()),
            _ => {}
        }
    }
    Some(FieldAttribute::Relation(RelationAttribute {
        fields,
        references,
        on_delete,
        on_update,
    }))
}

fn parse_identifier_list(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if let Some(inner) = trimmed.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        inner
            .split(',')
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        Vec::new()
    }
}

fn parse_default_value(inner: &str) -> DefaultValue {
    let value = inner.trim();
    match value {
        "now()" => DefaultValue::Now,
        "autoincrement()" => DefaultValue::AutoIncrement,
        "uuid()" => DefaultValue::Uuid,
        _ => {
            if let Some(arg) = value
                .strip_prefix("dbgenerated(")
                .and_then(|v| v.strip_suffix(')'))
            {
                if let Some(lit) = parse_string_literal(arg.trim()) {
                    DefaultValue::DbGenerated(lit)
                } else {
                    DefaultValue::Expression(value.to_string())
                }
            } else {
                DefaultValue::Expression(value.to_string())
            }
        }
    }
}

fn parse_block_attribute(stmt: &str) -> BlockAttribute {
    let trimmed = stmt.trim();
    if trimmed.starts_with("@@map(") && trimmed.ends_with(')') {
        parse_string_argument(trimmed, "@@map(")
            .map(BlockAttribute::Map)
            .unwrap_or_else(|| BlockAttribute::Raw(trimmed.to_string()))
    } else {
        BlockAttribute::Raw(trimmed.to_string())
    }
}

fn split_type_and_attrs(s: &str) -> (String, Option<String>) {
    let mut in_string = false;
    let mut escape = false;
    let mut paren = 0;
    let mut bracket = 0;
    let mut index = s.len();
    for (i, ch) in s.char_indices() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => paren += 1,
            ')' => {
                if paren > 0 {
                    paren -= 1;
                }
            }
            '[' => bracket += 1,
            ']' => {
                if bracket > 0 {
                    bracket -= 1;
                }
            }
            '@' if paren == 0 && bracket == 0 => {
                index = i;
                break;
            }
            _ => {}
        }
    }
    if index == s.len() {
        (s.trim().to_string(), None)
    } else {
        let type_part = s[..index].trim().to_string();
        let attrs_part = s[index..].trim();
        if attrs_part.is_empty() {
            (type_part, None)
        } else {
            (type_part, Some(attrs_part.to_string()))
        }
    }
}

fn split_statements(body: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escape = false;
    let mut paren = 0;
    let mut bracket = 0;

    for ch in body.chars() {
        current.push(ch);
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => paren += 1,
            ')' => {
                if paren > 0 {
                    paren -= 1;
                }
            }
            '[' => bracket += 1,
            ']' => {
                if bracket > 0 {
                    bracket -= 1;
                }
            }
            '\n' => {
                if paren == 0 && bracket == 0 {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        statements.push(trimmed.to_string());
                    }
                    current.clear();
                }
            }
            _ => {}
        }
    }
    if !current.trim().is_empty() {
        statements.push(current.trim().to_string());
    }
    statements
}

fn split_attributes(s: &str) -> Vec<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut attrs = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escape = false;
    let mut paren = 0;
    let mut bracket = 0;

    for ch in trimmed.chars() {
        if current.is_empty() && ch.is_whitespace() {
            continue;
        }
        if ch == '@' && !in_string && paren == 0 && bracket == 0 {
            if !current.trim().is_empty() {
                attrs.push(current.trim().to_string());
            }
            current.clear();
        }
        current.push(ch);
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => paren += 1,
            ')' => {
                if paren > 0 {
                    paren -= 1;
                }
            }
            '[' => bracket += 1,
            ']' => {
                if bracket > 0 {
                    bracket -= 1;
                }
            }
            _ => {}
        }
    }
    if !current.trim().is_empty() {
        attrs.push(current.trim().to_string());
    }
    attrs
}

fn split_arguments(inner: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escape = false;
    let mut paren = 0;
    let mut bracket = 0;

    for ch in inner.chars() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            current.push(ch);
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                current.push(ch);
            }
            '(' => {
                paren += 1;
                current.push(ch);
            }
            ')' => {
                if paren > 0 {
                    paren -= 1;
                }
                current.push(ch);
            }
            '[' => {
                bracket += 1;
                current.push(ch);
            }
            ']' => {
                if bracket > 0 {
                    bracket -= 1;
                }
                current.push(ch);
            }
            ',' if paren == 0 && bracket == 0 => {
                if !current.trim().is_empty() {
                    args.push(current.trim().to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        args.push(current.trim().to_string());
    }
    args
}

fn parse_string_argument(attr: &str, prefix: &str) -> Option<String> {
    attr.strip_prefix(prefix)
        .and_then(|s| s.strip_suffix(')'))
        .and_then(|inner| parse_string_literal(inner.trim()))
}

fn parse_string_literal(lit: &str) -> Option<String> {
    let trimmed = lit.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') {
        serde_json::from_str::<String>(trimmed).ok()
    } else {
        None
    }
}

fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape = false;

    while let Some(ch) = chars.next() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            out.push(ch);
            continue;
        }

        match ch {
            '"' => {
                in_string = true;
                out.push(ch);
            }
            '/' => match chars.peek() {
                Some('/') => {
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                }
                Some('*') => {
                    chars.next();
                    let mut prev = '\0';
                    while let Some(c) = chars.next() {
                        if prev == '*' && c == '/' {
                            break;
                        }
                        prev = c;
                    }
                }
                _ => out.push(ch),
            },
            _ => out.push(ch),
        }
    }

    out
}

fn skip_whitespace(chars: &[char], idx: &mut usize) {
    while *idx < chars.len() && chars[*idx].is_whitespace() {
        *idx += 1;
    }
}

fn matches_keyword(chars: &[char], idx: usize, keyword: &str) -> bool {
    if idx + keyword.len() > chars.len() {
        return false;
    }
    if chars[idx..idx + keyword.len()].iter().collect::<String>() != keyword {
        return false;
    }
    if idx > 0 {
        if let Some(prev) = chars.get(idx - 1) {
            if prev.is_alphanumeric() || *prev == '_' {
                return false;
            }
        }
    }
    if let Some(next) = chars.get(idx + keyword.len()) {
        if next.is_alphanumeric() || *next == '_' {
            return false;
        }
    }
    true
}

fn parse_identifier(chars: &[char], idx: &mut usize) -> Result<String> {
    let start = *idx;
    while *idx < chars.len() {
        let ch = chars[*idx];
        if ch.is_alphanumeric() || ch == '_' {
            *idx += 1;
        } else {
            break;
        }
    }
    if *idx == start {
        bail!("expected identifier");
    }
    Ok(chars[start..*idx].iter().collect())
}

fn extract_block(chars: &[char], mut idx: usize) -> Result<(String, usize)> {
    let mut depth = 1;
    let mut out = String::new();
    let mut in_string = false;
    let mut escape = false;
    while idx < chars.len() {
        let ch = chars[idx];
        idx += 1;
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            out.push(ch);
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                out.push(ch);
            }
            '{' => {
                depth += 1;
                out.push(ch);
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok((out, idx));
                }
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    Err(anyhow!("unterminated block"))
}
