use super::{LintCheck, LintMessage, LintSeverity};
use crate::ir::Config;

pub struct LongIdentifier;

const MAX_IDENTIFIER_LEN: usize = 63;

impl LongIdentifier {
    fn ignored(ignores: &[String], rule: &str) -> bool {
        ignores.iter().any(|i| i == rule)
    }
}

impl LintCheck for LongIdentifier {
    fn name(&self) -> &'static str {
        "long-identifier"
    }

    fn run(&self, cfg: &Config) -> Vec<LintMessage> {
        let mut msgs = Vec::new();
        for table in &cfg.tables {
            if Self::ignored(&table.lint_ignore, self.name()) {
                continue;
            }
            if table.name.len() > MAX_IDENTIFIER_LEN {
                msgs.push(LintMessage {
                    check: self.name(),
                    message: format!(
                        "table '{}' name exceeds {} characters",
                        table.name, MAX_IDENTIFIER_LEN
                    ),
                    severity: LintSeverity::Error,
                });
            }
            for col in &table.columns {
                if Self::ignored(&col.lint_ignore, self.name()) {
                    continue;
                }
                if col.name.len() > MAX_IDENTIFIER_LEN {
                    msgs.push(LintMessage {
                        check: self.name(),
                        message: format!(
                            "column '{}.{}' name exceeds {} characters",
                            table.name, col.name, MAX_IDENTIFIER_LEN
                        ),
                        severity: LintSeverity::Error,
                    });
                }
            }
            for idx in &table.indexes {
                if let Some(name) = &idx.name {
                    if name.len() > MAX_IDENTIFIER_LEN {
                        msgs.push(LintMessage {
                            check: self.name(),
                            message: format!(
                                "index '{}.{}' name exceeds {} characters",
                                table.name, name, MAX_IDENTIFIER_LEN
                            ),
                            severity: LintSeverity::Error,
                        });
                    }
                }
            }
        }
        for idx in &cfg.indexes {
            if idx.name.len() > MAX_IDENTIFIER_LEN {
                msgs.push(LintMessage {
                    check: self.name(),
                    message: format!(
                        "index '{}' name exceeds {} characters",
                        idx.name, MAX_IDENTIFIER_LEN
                    ),
                    severity: LintSeverity::Error,
                });
            }
        }
        msgs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{ColumnSpec, Config, PrimaryKeySpec, TableSpec};
    use crate::lint::{run_with_checks, LintSettings};

    #[test]
    fn detects_long_name() {
        let long_name = "a".repeat(64);
        let table = TableSpec {
            name: long_name.clone(),
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
            indexes: vec![],
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
        let msgs = run_with_checks(
            &cfg,
            vec![Box::new(LongIdentifier)],
            &LintSettings::default(),
        );
        assert!(msgs.iter().any(|m| m.check == "long-identifier"));
    }
}
