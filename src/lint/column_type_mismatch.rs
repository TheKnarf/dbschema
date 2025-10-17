use super::{LintCheck, LintMessage, LintSeverity};
use crate::ir::{Config, TableSpec};

pub struct ColumnTypeMismatch;

impl ColumnTypeMismatch {
    fn ignored(table: &TableSpec, column: &str, rule: &str) -> bool {
        if table.lint_ignore.iter().any(|i| i == rule) {
            return true;
        }
        if let Some(col) = table.columns.iter().find(|c| &c.name == column) {
            if col.lint_ignore.iter().any(|i| i == rule) {
                return true;
            }
        }
        false
    }
}

impl LintCheck for ColumnTypeMismatch {
    fn name(&self) -> &'static str {
        "column-type-mismatch"
    }

    fn run(&self, cfg: &Config) -> Vec<LintMessage> {
        let mut msgs = Vec::new();
        for table in &cfg.tables {
            for fk in &table.foreign_keys {
                for (col_name, ref_col_name) in fk.columns.iter().zip(&fk.ref_columns) {
                    if Self::ignored(table, col_name, self.name()) {
                        continue;
                    }
                    let Some(src_col) = table.columns.iter().find(|c| &c.name == col_name) else {
                        continue;
                    };
                    let Some(ref_table) = cfg
                        .tables
                        .iter()
                        .find(|t| t.alt_name.as_ref().unwrap_or(&t.name) == &fk.ref_table)
                    else {
                        continue;
                    };
                    let Some(ref_col) = ref_table.columns.iter().find(|c| &c.name == ref_col_name)
                    else {
                        continue;
                    };
                    let src_ty = src_col
                        .db_type
                        .as_deref()
                        .unwrap_or(&src_col.r#type)
                        .to_lowercase();
                    let ref_ty = ref_col
                        .db_type
                        .as_deref()
                        .unwrap_or(&ref_col.r#type)
                        .to_lowercase();
                    if src_ty != ref_ty {
                        msgs.push(LintMessage {
                            check: self.name(),
                            message: format!(
                                "column '{}.{}' type '{}' does not match '{}.{}' type '{}'",
                                table.name, col_name, src_ty, ref_table.name, ref_col_name, ref_ty
                            ),
                            severity: LintSeverity::Error,
                        });
                    }
                }
            }
        }
        msgs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{ColumnSpec, Config, ForeignKeySpec, TableSpec};
    use crate::lint::{LintSettings, run_with_checks};

    #[test]
    fn detects_type_mismatch() {
        let referenced = TableSpec {
            name: "ref".into(),
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
            primary_key: None,
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
        let table = TableSpec {
            name: "t".into(),
            alt_name: None,
            schema: None,
            if_not_exists: false,
            columns: vec![ColumnSpec {
                name: "ref_id".into(),
                r#type: "text".into(),
                nullable: false,
                default: None,
                db_type: None,
                lint_ignore: vec![],
                comment: None,
                count: 1,
            }],
            primary_key: None,
            indexes: vec![],
            checks: vec![],
            foreign_keys: vec![ForeignKeySpec {
                name: None,
                columns: vec!["ref_id".into()],
                ref_schema: None,
                ref_table: "ref".into(),
                ref_columns: vec!["id".into()],
                on_delete: None,
                on_update: None,
                back_reference_name: None,
            }],
            partition_by: None,
            partitions: vec![],
            back_references: vec![],
            lint_ignore: vec![],
            comment: None,
            map: None,
        };
        let cfg = Config {
            tables: vec![referenced, table],
            ..Default::default()
        };
        let msgs = run_with_checks(
            &cfg,
            vec![Box::new(ColumnTypeMismatch)],
            &LintSettings::default(),
        );
        assert!(msgs.iter().any(|m| m.check == "column-type-mismatch"));
    }
}
