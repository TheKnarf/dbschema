use anyhow::{anyhow, Context, Result};
use postgres::{Client, NoTls, Row};
use std::collections::HashSet;

use super::{TestBackend, TestResult, TestSummary};
use crate::ir::Config;

pub struct PostgresTestBackend;

impl TestBackend for PostgresTestBackend {
    fn run(&self, cfg: &Config, dsn: &str, only: Option<&HashSet<String>>) -> Result<TestSummary> {
        let mut client = Client::connect(dsn, NoTls)
            .with_context(|| format!("connecting to database: {}", redacted(dsn)))?;
        let mut results = Vec::new();
        let mut passed = 0usize;
        for t in &cfg.tests {
            if let Some(only) = only {
                if !only.contains(&t.name) {
                    continue;
                }
            }
            let name = t.name.clone();
            let mut tx = client.transaction()?;
            let mut failed_msg = String::new();
            let mut ok = true;
            for s in &t.setup {
                if let Err(e) = tx.batch_execute(s) {
                    failed_msg = format!("setup failed: {}", e);
                    ok = false;
                    break;
                }
            }
            if ok {
                match tx.query(&t.assert_sql, &[]) {
                    Ok(rows) => {
                        match assert_rows_true(&rows) {
                            Ok(true) => { /* ok */ }
                            Ok(false) => {
                                ok = false;
                                failed_msg = "assert returned false".into();
                            }
                            Err(e) => {
                                ok = false;
                                failed_msg = format!("assert error: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        ok = false;
                        failed_msg = format!("assert query error: {}", e);
                    }
                }
            }
            // Always rollback to keep DB clean
            let _ = tx.rollback();
            if ok {
                passed += 1;
            }
            results.push(TestResult {
                name,
                passed: ok,
                message: if ok { "ok".into() } else { failed_msg },
            });
        }
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

type Converter = fn(&Row) -> Result<bool>;

macro_rules! converters {
    ($($src:ty => $map:expr),+ $(,)?) => {
        &[
            $(|row: &Row| row
                .try_get::<usize, $src>(0)
                .map($map)
                .map_err(Into::into),)+
        ]
    };
}

static CONVERTERS: &[Converter] = converters!(
    bool => |v| v,
    i64 => |v| v != 0,
    i32 => |v| v != 0,
    i16 => |v| v != 0,
    i8 => |v| v != 0,
    i64 => |v| (v as u64) != 0,
    u32 => |v| v != 0,
    i16 => |v| (v as u16) != 0,
    i8 => |v| (v as u8) != 0,
    String => |v| v == "t" || v.eq_ignore_ascii_case("true"),
);

/// Evaluate the first column of the first row for truthiness.
///
/// Supported types:
/// * `bool`
/// * any signed or unsigned integer (non-zero is treated as `true`)
/// * text values "t" or "true" (case-insensitive)
fn assert_rows_true(rows: &[Row]) -> Result<bool> {
    if rows.is_empty() {
        return Ok(false);
    }
    let cols = rows[0].columns();
    if cols.is_empty() {
        return Ok(false);
    }
    let row = &rows[0];
    for conv in CONVERTERS {
        if let Ok(v) = conv(row) {
            return Ok(v);
        }
    }
    Err(anyhow!("unsupported assert result type"))
}

fn redacted(dsn: &str) -> String {
    // Very basic redaction (remove password part after : )
    if let Some(idx) = dsn.find("@") {
        let (left, right) = dsn.split_at(idx);
        if let Some(colon) = left.find(":") {
            let (scheme_user, _rest) = left.split_at(colon + 1);
            return format!("{}****{}", scheme_user, right);
        }
    }
    dsn.to_string()
}
