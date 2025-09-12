use std::fmt;

use crate::postgres::{ident, literal};

#[derive(Debug, Clone)]
pub struct ForeignServer {
    pub name: String,
    pub wrapper: String,
    pub r#type: Option<String>,
    pub version: Option<String>,
    pub options: Vec<String>,
}

impl From<&crate::ir::ForeignServerSpec> for ForeignServer {
    fn from(s: &crate::ir::ForeignServerSpec) -> Self {
        Self {
            name: s.alt_name.clone().unwrap_or_else(|| s.name.clone()),
            wrapper: s.wrapper.clone(),
            r#type: s.r#type.clone(),
            version: s.version.clone(),
            options: s.options.clone(),
        }
    }
}

impl fmt::Display for ForeignServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CREATE SERVER {}", ident(&self.name))?;
        if let Some(t) = &self.r#type {
            write!(f, " TYPE {}", literal(t))?;
        }
        if let Some(v) = &self.version {
            write!(f, " VERSION {}", literal(v))?;
        }
        write!(f, " FOREIGN DATA WRAPPER {}", ident(&self.wrapper))?;
        if !self.options.is_empty() {
            write!(f, " OPTIONS ({})", self.options.join(", "))?;
        }
        write!(f, ";")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_basic() {
        let spec = crate::ir::ForeignServerSpec {
            name: "srv".into(),
            alt_name: None,
            wrapper: "fdw".into(),
            r#type: Some("postgres".into()),
            version: None,
            options: vec!["host 'localhost'".into()],
            comment: None,
        };
        let srv = ForeignServer::from(&spec);
        assert_eq!(
            srv.to_string(),
            "CREATE SERVER \"srv\" TYPE 'postgres' FOREIGN DATA WRAPPER \"fdw\" OPTIONS (host 'localhost');",
        );
    }
}
