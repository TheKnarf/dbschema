use std::fmt;

use crate::postgres::{ident, literal};

#[derive(Debug, Clone)]
pub struct Collation {
    pub schema: String,
    pub name: String,
    pub if_not_exists: bool,
    pub from: Option<String>,
    pub locale: Option<String>,
    pub lc_collate: Option<String>,
    pub lc_ctype: Option<String>,
    pub provider: Option<String>,
    pub deterministic: Option<bool>,
    pub version: Option<String>,
}

impl From<&crate::ir::CollationSpec> for Collation {
    fn from(c: &crate::ir::CollationSpec) -> Self {
        Self {
            schema: c.schema.clone().unwrap_or_else(|| "public".to_string()),
            name: c.alt_name.clone().unwrap_or_else(|| c.name.clone()),
            if_not_exists: c.if_not_exists,
            from: c.from.clone(),
            locale: c.locale.clone(),
            lc_collate: c.lc_collate.clone(),
            lc_ctype: c.lc_ctype.clone(),
            provider: c.provider.clone(),
            deterministic: c.deterministic,
            version: c.version.clone(),
        }
    }
}

impl fmt::Display for Collation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CREATE COLLATION")?;
        if self.if_not_exists {
            write!(f, " IF NOT EXISTS")?;
        }
        write!(f, " {}.{}", ident(&self.schema), ident(&self.name))?;
        if let Some(from) = &self.from {
            write!(f, " FROM {from}")?;
        } else {
            let mut parts = Vec::new();
            if let Some(locale) = &self.locale {
                parts.push(format!("LOCALE = {}", literal(locale)));
            }
            if let Some(lc_collate) = &self.lc_collate {
                parts.push(format!("LC_COLLATE = {}", literal(lc_collate)));
            }
            if let Some(lc_ctype) = &self.lc_ctype {
                parts.push(format!("LC_CTYPE = {}", literal(lc_ctype)));
            }
            if let Some(provider) = &self.provider {
                parts.push(format!("PROVIDER = {}", provider.to_uppercase()));
            }
            if let Some(det) = self.deterministic {
                parts.push(format!(
                    "DETERMINISTIC = {}",
                    if det { "true" } else { "false" }
                ));
            }
            if let Some(version) = &self.version {
                parts.push(format!("VERSION = {}", literal(version)));
            }
            if !parts.is_empty() {
                write!(f, " ({})", parts.join(", "))?;
            }
        }
        write!(f, ";")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collation_with_locale() {
        let spec = crate::ir::CollationSpec {
            name: "c".into(),
            alt_name: None,
            schema: None,
            if_not_exists: true,
            from: None,
            locale: Some("en_US".into()),
            lc_collate: None,
            lc_ctype: None,
            provider: None,
            deterministic: None,
            version: None,
            comment: None,
        };
        let coll = Collation::from(&spec);
        assert_eq!(
            coll.to_string(),
            "CREATE COLLATION IF NOT EXISTS \"public\".\"c\" (LOCALE = 'en_US');"
        );
    }
}
