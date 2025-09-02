use crate::ir::Config;

#[derive(Debug)]
pub struct LintMessage {
    pub check: &'static str,
    pub message: String,
}

pub trait LintCheck {
    fn name(&self) -> &'static str;
    fn run(&self, cfg: &Config) -> Vec<LintMessage>;
}

pub fn run(cfg: &Config) -> Vec<LintMessage> {
    let checks: Vec<Box<dyn LintCheck>> = vec![
        Box::new(NamingConvention),
        Box::new(MissingIndex),
    ];
    run_with_checks(cfg, checks)
}

pub fn run_with_checks(cfg: &Config, checks: Vec<Box<dyn LintCheck>>) -> Vec<LintMessage> {
    let mut messages = Vec::new();
    for check in checks {
        messages.extend(check.run(cfg));
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
            if table.indexes.is_empty() && table.primary_key.is_none() {
                msgs.push(LintMessage {
                    check: self.name(),
                    message: format!("table '{}' has no indexes", table.name),
                });
            }
        }
        msgs
    }
}
