use super::{LintCheck, LintMessage, LintSeverity};
use crate::ir::Config;

pub struct SqlSyntax;

impl SqlSyntax {
    fn push_err(&self, msgs: &mut Vec<LintMessage>, ctx: &str, err: pg_query::Error) {
        msgs.push(LintMessage {
            check: self.name(),
            message: format!("invalid SQL in {}: {}", ctx, err),
            severity: LintSeverity::Error,
        });
    }

    fn check_stmt(&self, msgs: &mut Vec<LintMessage>, sql: &str, ctx: &str) {
        if let Err(err) = pg_query::parse(sql) {
            self.push_err(msgs, ctx, err);
        }
    }

    fn check_expr(&self, msgs: &mut Vec<LintMessage>, expr: &str, ctx: &str) {
        if let Err(err) = pg_query::parse(&format!("SELECT {}", expr)) {
            self.push_err(msgs, ctx, err);
        }
    }
}

impl LintCheck for SqlSyntax {
    fn name(&self) -> &'static str {
        "sql-syntax"
    }

    fn run(&self, cfg: &Config) -> Vec<LintMessage> {
        let mut msgs = Vec::new();

        for view in &cfg.views {
            self.check_stmt(&mut msgs, &view.sql, &format!("view '{}'", view.name));
        }
        for mview in &cfg.materialized {
            self.check_stmt(
                &mut msgs,
                &mview.sql,
                &format!("materialized view '{}'", mview.name),
            );
        }
        for policy in &cfg.policies {
            if let Some(using) = &policy.using {
                self.check_expr(&mut msgs, using, &format!("policy '{}' USING", policy.name));
            }
            if let Some(check) = &policy.check {
                self.check_expr(&mut msgs, check, &format!("policy '{}' CHECK", policy.name));
            }
        }
        for table in &cfg.tables {
            for chk in &table.checks {
                self.check_expr(
                    &mut msgs,
                    &chk.expression,
                    &format!("table '{}' CHECK", table.name),
                );
            }
        }
        for domain in &cfg.domains {
            if let Some(expr) = &domain.constraint {
                self.check_expr(
                    &mut msgs,
                    expr,
                    &format!("domain '{}' CONSTRAINT", domain.name),
                );
            }
            if let Some(expr) = &domain.check {
                self.check_expr(&mut msgs, expr, &format!("domain '{}' CHECK", domain.name));
            }
        }
        for trig in &cfg.triggers {
            if let Some(when) = &trig.when {
                self.check_expr(&mut msgs, when, &format!("trigger '{}' WHEN", trig.name));
            }
        }
        for func in &cfg.functions {
            if func.language.to_lowercase() == "sql" {
                self.check_stmt(&mut msgs, &func.body, &format!("function '{}'", func.name));
            }
        }
        for test in &cfg.tests {
            for stmt in test
                .setup
                .iter()
                .chain(&test.asserts)
                .chain(&test.assert_fail)
                .chain(&test.teardown)
            {
                self.check_stmt(&mut msgs, stmt, &format!("test '{}'", test.name));
            }
        }

        msgs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Config, ViewSpec};
    use crate::lint::{run_with_checks, LintSettings};

    #[test]
    fn detects_invalid_sql() {
        let view = ViewSpec {
            name: "v".into(),
            alt_name: None,
            schema: None,
            replace: false,
            sql: "SELEC 1".into(),
            comment: None,
        };
        let cfg = Config {
            views: vec![view],
            ..Default::default()
        };
        let msgs = run_with_checks(&cfg, vec![Box::new(SqlSyntax)], &LintSettings::default());
        assert!(msgs.iter().any(|m| m.check == "sql-syntax"));
    }
}
