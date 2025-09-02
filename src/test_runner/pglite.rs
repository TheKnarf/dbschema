use anyhow::Result;
use postgres_protocol::message::backend;
use std::collections::HashSet;

use super::{TestBackend, TestResult, TestSummary};
use crate::ir::Config;
use pglite::assert_row_true;

pub use pglite::PGliteRuntime;

/// Test backend powered by the in-memory PGlite runtime.
pub struct PGliteTestBackend;

impl TestBackend for PGliteTestBackend {
    fn run(&self, cfg: &Config, _dsn: &str, only: Option<&HashSet<String>>) -> Result<TestSummary> {
        let mut rt = pglite::PGliteRuntime::new()?;
        rt.startup()?;
        let mut results = Vec::new();
        let mut passed = 0usize;
        for t in &cfg.tests {
            if let Some(only) = only {
                if !only.contains(&t.name) {
                    continue;
                }
            }
            let name = t.name.clone();
            let mut ok = true;
            let mut failed_msg = String::new();
            for s in &t.setup {
                if let Err(e) = rt.simple_query(s) {
                    ok = false;
                    failed_msg = format!("setup failed: {}", e);
                    break;
                }
            }
            if ok {
                match rt.simple_query(&t.assert_sql) {
                    Ok(msgs) => {
                        let mut data_row = None;
                        for m in msgs {
                            if let backend::Message::DataRow(row) = m {
                                data_row = Some(row);
                            }
                        }
                        if let Some(row) = data_row {
                            match assert_row_true(&row) {
                                Ok(true) => {}
                                Ok(false) => {
                                    ok = false;
                                    failed_msg = "assert returned false".into();
                                }
                                Err(e) => {
                                    ok = false;
                                    failed_msg = format!("assert error: {}", e);
                                }
                            }
                        } else {
                            ok = false;
                            failed_msg = "assert returned no rows".into();
                        }
                    }
                    Err(e) => {
                        ok = false;
                        failed_msg = format!("assert query error: {}", e);
                    }
                }
            }
            if ok {
                passed += 1;
            }
            results.push(TestResult {
                name,
                passed: ok,
                message: if ok { "ok".into() } else { failed_msg },
            });
        }
        rt.shutdown()?;
        let total = results.len();
        let failed = total - passed;
        Ok(TestSummary {
            total,
            passed,
            failed,
            results,
        })
    }
}
