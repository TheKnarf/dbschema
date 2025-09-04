use super::{LintCheck, LintMessage, LintSeverity};
use crate::ir::{Config, ForeignKeySpec};

pub struct DestructiveChange;

impl DestructiveChange {
    fn ignored(ignores: &[String], rule: &str) -> bool {
        ignores.iter().any(|i| i == rule)
    }

    fn is_destructive_fk(fk: &ForeignKeySpec) -> bool {
        matches!(fk.on_delete.as_deref(), Some(action) if action.eq_ignore_ascii_case("cascade"))
            || matches!(fk.on_update.as_deref(), Some(action) if action.eq_ignore_ascii_case("cascade"))
    }
}

impl LintCheck for DestructiveChange {
    fn name(&self) -> &'static str {
        "destructive-change"
    }

    fn run(&self, cfg: &Config) -> Vec<LintMessage> {
        let mut msgs = Vec::new();
        for table in &cfg.tables {
            if Self::ignored(&table.lint_ignore, self.name()) {
                continue;
            }
            for fk in &table.foreign_keys {
                if Self::is_destructive_fk(fk) {
                    msgs.push(LintMessage {
                        check: self.name(),
                        message: format!(
                            "foreign key on '{}.{}' uses CASCADE action",
                            table.name,
                            fk.columns.join(",")
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
    use crate::ir::{ColumnSpec, Config, ForeignKeySpec, PrimaryKeySpec, TableSpec};
    use crate::lint::{run_with_checks, LintSettings};

    #[test]
    fn detects_cascade_fk() {
        let table = TableSpec {
            name: "t".into(),
            table_name: None,
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
            }],
            primary_key: Some(PrimaryKeySpec {
                name: None,
                columns: vec!["id".into()],
            }),
            indexes: vec![],
            checks: vec![],
            foreign_keys: vec![ForeignKeySpec {
                name: None,
                columns: vec!["id".into()],
                ref_schema: None,
                ref_table: "other".into(),
                ref_columns: vec!["id".into()],
                on_delete: Some("cascade".into()),
                on_update: None,
                back_reference_name: None,
            }],
            back_references: vec![],
            lint_ignore: vec![],
            comment: None,
        };
        let cfg = Config {
            tables: vec![table],
            ..Default::default()
        };
        let msgs = run_with_checks(
            &cfg,
            vec![Box::new(DestructiveChange)],
            &LintSettings::default(),
        );
        assert!(msgs.iter().any(|m| m.check == "destructive-change"));
    }
}
