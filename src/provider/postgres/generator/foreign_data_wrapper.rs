use std::fmt;

use super::ident;

#[derive(Debug, Clone)]
pub struct ForeignDataWrapper {
    pub name: String,
    pub handler: Option<String>,
    pub validator: Option<String>,
    pub options: Vec<String>,
}

impl From<&crate::ir::ForeignDataWrapperSpec> for ForeignDataWrapper {
    fn from(f: &crate::ir::ForeignDataWrapperSpec) -> Self {
        Self {
            name: f.alt_name.clone().unwrap_or_else(|| f.name.clone()),
            handler: f.handler.clone(),
            validator: f.validator.clone(),
            options: f.options.clone(),
        }
    }
}

impl fmt::Display for ForeignDataWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CREATE FOREIGN DATA WRAPPER {}", ident(&self.name))?;
        if let Some(h) = &self.handler {
            write!(f, " HANDLER {h}")?;
        }
        if let Some(v) = &self.validator {
            write!(f, " VALIDATOR {v}")?;
        }
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
    fn fdw_basic() {
        let spec = crate::ir::ForeignDataWrapperSpec {
            name: "fdw".into(),
            alt_name: None,
            handler: Some("my_handler".into()),
            validator: None,
            options: vec!["host 'localhost'".into()],
            comment: None,
        };
        let fdw = ForeignDataWrapper::from(&spec);
        assert_eq!(
            fdw.to_string(),
            "CREATE FOREIGN DATA WRAPPER \"fdw\" HANDLER my_handler OPTIONS (host 'localhost');",
        );
    }
}
