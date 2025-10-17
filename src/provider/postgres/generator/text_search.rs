use std::fmt;

use super::ident;

#[derive(Debug, Clone)]
pub struct TextSearchDictionary {
    pub schema: String,
    pub name: String,
    pub template: String,
    pub options: Vec<String>,
}

impl From<&crate::ir::TextSearchDictionarySpec> for TextSearchDictionary {
    fn from(d: &crate::ir::TextSearchDictionarySpec) -> Self {
        Self {
            schema: d.schema.clone().unwrap_or_else(|| "public".to_string()),
            name: d.alt_name.clone().unwrap_or_else(|| d.name.clone()),
            template: d.template.clone(),
            options: d.options.clone(),
        }
    }
}

impl fmt::Display for TextSearchDictionary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CREATE TEXT SEARCH DICTIONARY {}.{} (TEMPLATE = {}",
            ident(&self.schema),
            ident(&self.name),
            self.template
        )?;
        for opt in &self.options {
            write!(f, ", {opt}")?;
        }
        write!(f, ");")
    }
}

#[derive(Debug, Clone)]
pub struct TextSearchConfigurationMapping {
    pub tokens: Vec<String>,
    pub dictionaries: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TextSearchConfiguration {
    pub schema: String,
    pub name: String,
    pub parser: String,
    pub mappings: Vec<TextSearchConfigurationMapping>,
}

impl From<&crate::ir::TextSearchConfigurationSpec> for TextSearchConfiguration {
    fn from(c: &crate::ir::TextSearchConfigurationSpec) -> Self {
        Self {
            schema: c.schema.clone().unwrap_or_else(|| "public".to_string()),
            name: c.alt_name.clone().unwrap_or_else(|| c.name.clone()),
            parser: c.parser.clone(),
            mappings: c
                .mappings
                .iter()
                .map(|m| TextSearchConfigurationMapping {
                    tokens: m.tokens.clone(),
                    dictionaries: m.dictionaries.clone(),
                })
                .collect(),
        }
    }
}

impl fmt::Display for TextSearchConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CREATE TEXT SEARCH CONFIGURATION {}.{} (PARSER = {});",
            ident(&self.schema),
            ident(&self.name),
            self.parser
        )?;
        for m in &self.mappings {
            write!(
                f,
                "\nALTER TEXT SEARCH CONFIGURATION {}.{} ADD MAPPING FOR {} WITH {};",
                ident(&self.schema),
                ident(&self.name),
                m.tokens.join(", "),
                m.dictionaries.join(", ")
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TextSearchTemplate {
    pub schema: String,
    pub name: String,
    pub init: Option<String>,
    pub lexize: String,
}

impl From<&crate::ir::TextSearchTemplateSpec> for TextSearchTemplate {
    fn from(t: &crate::ir::TextSearchTemplateSpec) -> Self {
        Self {
            schema: t.schema.clone().unwrap_or_else(|| "public".to_string()),
            name: t.alt_name.clone().unwrap_or_else(|| t.name.clone()),
            init: t.init.clone(),
            lexize: t.lexize.clone(),
        }
    }
}

impl fmt::Display for TextSearchTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CREATE TEXT SEARCH TEMPLATE {}.{} (LEXIZE = {}",
            ident(&self.schema),
            ident(&self.name),
            self.lexize
        )?;
        if let Some(init) = &self.init {
            write!(f, ", INIT = {init}")?;
        }
        write!(f, ");")
    }
}

#[derive(Debug, Clone)]
pub struct TextSearchParser {
    pub schema: String,
    pub name: String,
    pub start: String,
    pub gettoken: String,
    pub end: String,
    pub headline: Option<String>,
    pub lextypes: String,
}

impl From<&crate::ir::TextSearchParserSpec> for TextSearchParser {
    fn from(p: &crate::ir::TextSearchParserSpec) -> Self {
        Self {
            schema: p.schema.clone().unwrap_or_else(|| "public".to_string()),
            name: p.alt_name.clone().unwrap_or_else(|| p.name.clone()),
            start: p.start.clone(),
            gettoken: p.gettoken.clone(),
            end: p.end.clone(),
            headline: p.headline.clone(),
            lextypes: p.lextypes.clone(),
        }
    }
}

impl fmt::Display for TextSearchParser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CREATE TEXT SEARCH PARSER {}.{} (START = {}, GETTOKEN = {}, END = {}, LEXTYPES = {}",
            ident(&self.schema),
            ident(&self.name),
            self.start,
            self.gettoken,
            self.end,
            self.lextypes
        )?;
        if let Some(headline) = &self.headline {
            write!(f, ", HEADLINE = {headline}")?;
        }
        write!(f, ");")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_sql_basic() {
        let spec = crate::ir::TextSearchDictionarySpec {
            name: "d".into(),
            alt_name: None,
            schema: None,
            template: "simple".into(),
            options: vec!["dict = 'simple'".into()],
            comment: None,
        };
        let dict = TextSearchDictionary::from(&spec);
        assert_eq!(
            dict.to_string(),
            "CREATE TEXT SEARCH DICTIONARY \"public\".\"d\" (TEMPLATE = simple, dict = 'simple');"
        );
    }
}
