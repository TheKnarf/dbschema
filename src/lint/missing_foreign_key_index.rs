use super::{LintCheck, LintMessage, LintSeverity};
use crate::ir::{Config, TableSpec};

pub struct MissingForeignKeyIndex;

impl MissingForeignKeyIndex {
    fn ignored(table: &TableSpec, columns: &[String], rule: &str) -> bool {
        if table.lint_ignore.iter().any(|i| i == rule) {
            return true;
        }
        for col_name in columns {
            if let Some(col) = table.columns.iter().find(|c| &c.name == col_name) {
                if col.lint_ignore.iter().any(|i| i == rule) {
                    return true;
                }
            }
        }
        false
    }

    fn has_index(cfg: &Config, table: &TableSpec, columns: &[String]) -> bool {
        let matches = |idx_cols: &[String]| {
            if idx_cols.len() < columns.len() {
                return false;
            }
            idx_cols.iter().zip(columns).all(|(a, b)| a == b)
        };
        if let Some(pk) = &table.primary_key {
            if matches(&pk.columns) {
                return true;
            }
        }
        if table.indexes.iter().any(|i| matches(&i.columns)) {
            return true;
        }
        let tbl_name = table.alt_name.as_ref().unwrap_or(&table.name);
        let schema = table.schema.as_deref().unwrap_or("public");
        cfg.indexes
            .iter()
            .filter(|i| i.table == *tbl_name && i.schema.as_deref().unwrap_or("public") == schema)
            .any(|i| matches(&i.columns))
    }
}

impl LintCheck for MissingForeignKeyIndex {
    fn name(&self) -> &'static str {
        "missing-foreign-key-index"
    }

    fn run(&self, cfg: &Config) -> Vec<LintMessage> {
        let mut msgs = Vec::new();
        for table in &cfg.tables {
            for fk in &table.foreign_keys {
                if Self::ignored(table, &fk.columns, self.name()) {
                    continue;
                }
                if !Self::has_index(cfg, table, &fk.columns) {
                    msgs.push(LintMessage {
                        check: self.name(),
                        message: format!(
                            "foreign key on '{}.{}' has no index",
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
    use crate::ir::{ColumnSpec, Config, ForeignKeySpec, TableSpec};
    use crate::lint::{run_with_checks, LintSettings};

    #[test]
    fn detects_missing_fk_index() {
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
            vec![Box::new(MissingForeignKeyIndex)],
            &LintSettings::default(),
        );
        assert!(msgs.iter().any(|m| m.check == "missing-foreign-key-index"));
    }
}

