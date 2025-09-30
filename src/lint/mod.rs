use crate::ir::Config;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod destructive_change;
mod long_identifier;
mod column_type_mismatch;
#[cfg(feature = "postgres-backend")]
mod sql_syntax;
mod unused_index;
mod missing_foreign_key_index;

use destructive_change::DestructiveChange;
use long_identifier::LongIdentifier;
use column_type_mismatch::ColumnTypeMismatch;
#[cfg(feature = "postgres-backend")]
use sql_syntax::SqlSyntax;
use unused_index::UnusedIndex;
use missing_foreign_key_index::MissingForeignKeyIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintSeverity {
    Allow,
    Warn,
    Error,
}

#[derive(Debug)]
pub struct LintMessage {
    pub check: &'static str,
    pub message: String,
    pub severity: LintSeverity,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LintSettings {
    #[serde(default)]
    pub severity: HashMap<String, LintSeverity>,
}

pub trait LintCheck {
    fn name(&self) -> &'static str;
    fn run(&self, cfg: &Config) -> Vec<LintMessage>;
}

pub fn run(cfg: &Config, settings: &LintSettings) -> Vec<LintMessage> {
    let mut checks: Vec<Box<dyn LintCheck>> = vec![
        Box::new(NamingConvention),
        Box::new(MissingIndex),
        Box::new(MissingForeignKeyIndex),
        Box::new(ColumnTypeMismatch),
        Box::new(ForbidSerial),
        Box::new(PrimaryKeyNotNull),
        Box::new(DestructiveChange),
        Box::new(UnusedIndex),
        Box::new(LongIdentifier),
    ];
    #[cfg(feature = "postgres-backend")]
    checks.push(Box::new(SqlSyntax));
    run_with_checks(cfg, checks, settings)
}

pub fn run_with_checks(
    cfg: &Config,
    checks: Vec<Box<dyn LintCheck>>,
    settings: &LintSettings,
) -> Vec<LintMessage> {
    let mut messages = Vec::new();
    for check in checks {
        let severity = settings
            .severity
            .get(check.name())
            .copied()
            .unwrap_or(LintSeverity::Error);
        if severity == LintSeverity::Allow {
            continue;
        }
        for mut msg in check.run(cfg) {
            msg.severity = severity;
            messages.push(msg);
        }
    }
    messages
}

struct NamingConvention;

impl NamingConvention {
    fn is_snake_case(s: &str) -> bool {
        let mut chars = s.chars();
        match chars.next() {
            Some(c) if c.is_ascii_lowercase() || c == '_' => (),
            _ => return false,
        }
        chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    }

    fn ignored(ignores: &[String], rule: &str) -> bool {
        ignores.iter().any(|i| i == rule)
    }
}

impl LintCheck for NamingConvention {
    fn name(&self) -> &'static str {
        "naming-convention"
    }

    fn run(&self, cfg: &Config) -> Vec<LintMessage> {
        let mut msgs = Vec::new();
        for table in &cfg.tables {
            if Self::ignored(&table.lint_ignore, self.name()) {
                continue;
            }
            if !Self::is_snake_case(&table.name) {
                msgs.push(LintMessage {
                    check: self.name(),
                    message: format!("table '{}' should be snake_case", table.name),
                    severity: LintSeverity::Error,
                });
            }
            for col in &table.columns {
                if Self::ignored(&col.lint_ignore, self.name())
                    || Self::ignored(&table.lint_ignore, self.name())
                {
                    continue;
                }
                if !Self::is_snake_case(&col.name) {
                    msgs.push(LintMessage {
                        check: self.name(),
                        message: format!(
                            "column '{}.{}' should be snake_case",
                            table.name, col.name
                        ),
                        severity: LintSeverity::Error,
                    });
                }
            }
        }
        msgs
    }
}

struct MissingIndex;

impl MissingIndex {
    fn ignored(ignores: &[String], rule: &str) -> bool {
        ignores.iter().any(|i| i == rule)
    }
}

impl LintCheck for MissingIndex {
    fn name(&self) -> &'static str {
        "missing-index"
    }

    fn run(&self, cfg: &Config) -> Vec<LintMessage> {
        let mut msgs = Vec::new();
        for table in &cfg.tables {
            if Self::ignored(&table.lint_ignore, self.name()) {
                continue;
            }
            let tbl_name = table.alt_name.as_ref().unwrap_or(&table.name);
            let schema = table.schema.as_deref().unwrap_or("public");
            let has_global = cfg
                .indexes
                .iter()
                .any(|i| i.table == *tbl_name && i.schema.as_deref().unwrap_or("public") == schema);
            if table.indexes.is_empty() && !has_global && table.primary_key.is_none() {
                msgs.push(LintMessage {
                    check: self.name(),
                    message: format!("table '{}' has no indexes", table.name),
                    severity: LintSeverity::Error,
                });
            }
        }
        msgs
    }
}

struct ForbidSerial;

impl ForbidSerial {
    fn ignored(ignores: &[String], rule: &str) -> bool {
        ignores.iter().any(|i| i == rule)
    }
}

impl LintCheck for ForbidSerial {
    fn name(&self) -> &'static str {
        "forbid-serial"
    }

    fn run(&self, cfg: &Config) -> Vec<LintMessage> {
        let mut msgs = Vec::new();
        for table in &cfg.tables {
            if Self::ignored(&table.lint_ignore, self.name()) {
                continue;
            }
            for col in &table.columns {
                if Self::ignored(&col.lint_ignore, self.name())
                    || Self::ignored(&table.lint_ignore, self.name())
                {
                    continue;
                }
                if col.r#type.to_lowercase().contains("serial") {
                    msgs.push(LintMessage {
                        check: self.name(),
                        message: format!("column '{}.{}' uses serial type", table.name, col.name),
                        severity: LintSeverity::Error,
                    });
                }
            }
        }
        msgs
    }
}

struct PrimaryKeyNotNull;

impl PrimaryKeyNotNull {
    fn ignored(ignores: &[String], rule: &str) -> bool {
        ignores.iter().any(|i| i == rule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{ColumnSpec, Config, PrimaryKeySpec, TableSpec};

    fn base_table() -> Config {
        let table = TableSpec {
            name: "t".into(),
            alt_name: None,
            schema: None,
            if_not_exists: false,
            columns: vec![ColumnSpec {
                name: "id".into(),
                r#type: "serial".into(),
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
        Config {
            tables: vec![table],
            ..Default::default()
        }
    }

    #[test]
    fn forbid_serial_detected() {
        let cfg = base_table();
        let msgs = run(&cfg, &LintSettings::default());
        assert!(msgs.iter().any(|m| m.check == "forbid-serial"));
    }

    #[test]
    fn lint_ignore_suppresses_rule() {
        let mut cfg = base_table();
        cfg.tables[0].columns[0]
            .lint_ignore
            .push("forbid-serial".into());
        let msgs = run(&cfg, &LintSettings::default());
        assert!(msgs.iter().all(|m| m.check != "forbid-serial"));
    }

    #[test]
    fn severity_allow_suppresses_rule() {
        let cfg = base_table();
        let mut settings = LintSettings::default();
        settings
            .severity
            .insert("forbid-serial".into(), LintSeverity::Allow);
        let msgs = run(&cfg, &settings);
        assert!(msgs.is_empty());
    }

    #[test]
    fn primary_key_columns_must_be_not_null() {
        let mut cfg = base_table();
        cfg.tables[0].columns[0].nullable = true;
        let msgs = run(&cfg, &LintSettings::default());
        assert!(msgs.iter().any(|m| m.check == "primary-key-not-null"));
    }
}

impl LintCheck for PrimaryKeyNotNull {
    fn name(&self) -> &'static str {
        "primary-key-not-null"
    }

    fn run(&self, cfg: &Config) -> Vec<LintMessage> {
        let mut msgs = Vec::new();
        for table in &cfg.tables {
            let Some(pk) = &table.primary_key else {
                continue;
            };
            if Self::ignored(&table.lint_ignore, self.name()) {
                continue;
            }
            for col_name in &pk.columns {
                if let Some(col) = table.columns.iter().find(|c| &c.name == col_name) {
                    if Self::ignored(&col.lint_ignore, self.name())
                        || Self::ignored(&table.lint_ignore, self.name())
                    {
                        continue;
                    }
                    if col.nullable {
                        msgs.push(LintMessage {
                            check: self.name(),
                            message: format!(
                                "column '{}.{}' in primary key must be NOT NULL",
                                table.name, col.name
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
