use std::fmt;

use super::{ident, Column};

#[derive(Debug, Clone)]
pub struct ForeignTable {
    pub schema: String,
    pub name: String,
    pub server: String,
    pub columns: Vec<Column>,
    pub options: Vec<String>,
}

impl From<&crate::ir::ForeignTableSpec> for ForeignTable {
    fn from(t: &crate::ir::ForeignTableSpec) -> Self {
        Self {
            schema: t.schema.clone().unwrap_or_else(|| "public".to_string()),
            name: t.alt_name.clone().unwrap_or_else(|| t.name.clone()),
            server: t.server.clone(),
            columns: t.columns.iter().map(Into::into).collect(),
            options: t.options.clone(),
        }
    }
}

impl fmt::Display for ForeignTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cols = self
            .columns
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        write!(
            f,
            "CREATE FOREIGN TABLE {}.{} ({}) SERVER {}",
            ident(&self.schema),
            ident(&self.name),
            cols,
            ident(&self.server)
        )?;
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
    fn foreign_table_basic() {
        let spec = crate::ir::ForeignTableSpec {
            name: "ft".into(),
            alt_name: None,
            schema: Some("public".into()),
            server: "srv".into(),
            columns: vec![crate::ir::ColumnSpec {
                name: "id".into(),
                r#type: "int".into(),
                nullable: false,
                default: None,
                db_type: None,
                lint_ignore: vec![],
                comment: None,
                count: 1,
            }],
            options: vec!["schema_name 'public'".into()],
            comment: None,
        };
        let ft = ForeignTable::from(&spec);
        assert_eq!(
            ft.to_string(),
            "CREATE FOREIGN TABLE \"public\".\"ft\" (\"id\" int NOT NULL) SERVER \"srv\" OPTIONS (schema_name 'public');",
        );
    }
}
