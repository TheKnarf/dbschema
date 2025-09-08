use super::{LintCheck, LintMessage, LintSeverity};
use crate::ir::Config;

pub struct UnusedIndex;

impl UnusedIndex {
    fn ignored(ignores: &[String], rule: &str) -> bool {
        ignores.iter().any(|i| i == rule)
    }
}

impl LintCheck for UnusedIndex {
    fn name(&self) -> &'static str {
        "unused-index"
    }

    fn run(&self, cfg: &Config) -> Vec<LintMessage> {
        let mut msgs = Vec::new();
        for table in &cfg.tables {
            if Self::ignored(&table.lint_ignore, self.name()) {
                continue;
            }
            let pk_cols = table
                .primary_key
                .as_ref()
                .map(|pk| pk.columns.clone())
                .unwrap_or_default();
            for idx in &table.indexes {
                if idx.unique && idx.columns == pk_cols {
                    msgs.push(LintMessage {
                        check: self.name(),
                        message: format!(
                            "table '{}': index {:?} duplicates primary key",
                            table.name, idx.name
                        ),
                        severity: LintSeverity::Error,
                    });
                }
            }
        }
        for idx in &cfg.indexes {
            if let Some(table) = cfg
                .tables
                .iter()
                .find(|t| t.alt_name.as_ref().unwrap_or(&t.name) == &idx.table)
            {
                if Self::ignored(&table.lint_ignore, self.name()) {
                    continue;
                }
                let pk_cols = table
                    .primary_key
                    .as_ref()
                    .map(|pk| pk.columns.clone())
                    .unwrap_or_default();
                if idx.unique && idx.columns == pk_cols {
                    msgs.push(LintMessage {
                        check: self.name(),
                        message: format!(
                            "index '{}' duplicates primary key on table '{}'",
                            idx.name, table.name
                        ),
                        severity: LintSeverity::Error,
                    });
                }
            }
        }
        msgs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{ColumnSpec, Config, IndexSpec, PrimaryKeySpec, TableSpec};
    use crate::lint::{run_with_checks, LintSettings};

    #[test]
    fn detects_duplicate_index() {
        let table = TableSpec {
            name: "t".into(),
            alt_name: None,
            schema: None,
            if_not_exists: false,
            columns: vec![ColumnSpec {
                name: "id".into(),
                r#type: "int".into(),
                nullable: false,
                default: None,
                db_type: None,
                lint_ignore: vec![],
                comment: None,
                count: 1,
            }],
            primary_key: Some(PrimaryKeySpec {
                name: None,
                columns: vec!["id".into()],
            }),
            indexes: vec![IndexSpec {
                name: Some("idx".into()),
                columns: vec!["id".into()],
                expressions: vec![],
                r#where: None,
                orders: vec![],
                operator_classes: vec![],
                unique: true,
            }],
            checks: vec![],
            foreign_keys: vec![],
            partition_by: None,
            partitions: vec![],
            back_references: vec![],
            lint_ignore: vec![],
            comment: None,
            map: None,
        };
        let cfg = Config {
            tables: vec![table],
            ..Default::default()
        };
        let msgs = run_with_checks(&cfg, vec![Box::new(UnusedIndex)], &LintSettings::default());
        assert!(msgs.iter().any(|m| m.check == "unused-index"));
    }
}
