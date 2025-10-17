use anyhow::{Context, Result, anyhow};
use postgres::{Client, NoTls, Row};
use std::collections::HashSet;
use url::Url;

use crate::ir::Config;
use crate::test_runner::{TestBackend, TestResult, TestSummary, is_verbose};
use log::info;

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
                if is_verbose() {
                    info!("-- setup: {}", s);
                }
                if let Err(e) = tx.batch_execute(s) {
                    failed_msg = format!("setup failed: {}", e);
                    ok = false;
                    break;
                }
            }
            if ok {
                // Positive asserts
                for a in &t.asserts {
                    if is_verbose() {
                        info!("-- assert: {}", a);
                    }
                    match tx.query(a, &[]) {
                        Ok(rows) => match assert_rows_true(&rows) {
                            Ok(true) => {}
                            Ok(false) => {
                                ok = false;
                                failed_msg = "assert returned false".into();
                                break;
                            }
                            Err(e) => {
                                ok = false;
                                failed_msg = format!("assert error: {}", e);
                                break;
                            }
                        },
                        Err(e) => {
                            ok = false;
                            failed_msg = format!("assert query error: {}", e);
                            break;
                        }
                    }
                }
            }
            if ok {
                // Negative asserts expected to fail
                for a in &t.assert_fail {
                    if is_verbose() {
                        info!("-- assert-fail: {}", a);
                    }
                    match tx.batch_execute(a) {
                        Ok(_) => {
                            ok = false;
                            failed_msg = "assert-fail succeeded unexpectedly".into();
                            break;
                        }
                        Err(_) => {
                            // expected failure
                        }
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

    fn supports_temporary_database(&self) -> bool {
        true
    }

    fn setup_temporary_database(&self, dsn: &str, database_name: &str, verbose: bool) -> Result<String> {
        let mut base =
            Url::parse(dsn).with_context(|| format!("parsing DSN as URL: {}", redacted(dsn)))?;
        let mut admin_base = base.clone();
        admin_base.set_path("/postgres");
        let admin_dsn = admin_base.as_str().to_string();
        let mut admin = Client::connect(&admin_dsn, NoTls)
            .with_context(|| format!("connecting to admin database: {}", redacted(&admin_dsn)))?;
        if verbose {
            info!("-- admin: DROP DATABASE IF EXISTS \"{}\";", database_name);
        }
        admin
            .simple_query(&format!("DROP DATABASE IF EXISTS \"{}\";", database_name))
            .with_context(|| format!("dropping database '{}'", database_name))?;
        if verbose {
            info!("-- admin: CREATE DATABASE \"{}\";", database_name);
        }
        admin
            .simple_query(&format!("CREATE DATABASE \"{}\";", database_name))
            .with_context(|| format!("creating database '{}'", database_name))?;
        base.set_path(&format!("/{}", database_name));
        Ok(base.as_str().to_string())
    }

    fn cleanup_temporary_database(&self, dsn: &str, database_name: &str, verbose: bool) -> Result<()> {
        if let Ok(mut base) = Url::parse(dsn) {
            base.set_path("/postgres");
            let admin_dsn = base.as_str().to_string();
            if let Ok(mut admin) = Client::connect(&admin_dsn, NoTls) {
                if verbose {
                    info!("-- admin: DROP DATABASE IF EXISTS \"{}\";", database_name);
                }
                let _ = admin.simple_query(&format!("DROP DATABASE IF EXISTS \"{}\";", database_name));
            }
        }
        Ok(())
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
    match Url::parse(dsn) {
        Ok(mut url) => {
            if url.password().is_some() {
                // ignore result since failure to set password is non-fatal
                let _ = url.set_password(Some("****"));
            }
            url.to_string()
        }
        Err(_) => dsn.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::redacted;

    #[test]
    fn masks_password() {
        let dsn = "postgres://user:secret@localhost:5432/db";
        assert_eq!(redacted(dsn), "postgres://user:****@localhost:5432/db");
    }

    #[test]
    fn preserves_query_and_port() {
        let dsn = "postgresql://user:secret@localhost:5432/db?sslmode=require";
        assert_eq!(
            redacted(dsn),
            "postgresql://user:****@localhost:5432/db?sslmode=require"
        );
    }

    #[test]
    fn leaves_without_password() {
        let dsn = "postgres://user@localhost/db";
        assert_eq!(redacted(dsn), dsn);
    }

    #[test]
    fn falls_back_on_parse_failure() {
        let dsn = "host=localhost user=me password=secret";
        assert_eq!(redacted(dsn), dsn);
    }
}
