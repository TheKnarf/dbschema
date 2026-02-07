use anyhow::{Context, Result, anyhow};
use fallible_iterator::FallibleIterator;
use postgres::{Client, NoTls, Row, Transaction};
use std::collections::HashSet;
use std::time::Duration;
use url::Url;

use crate::ir::{Config, InvariantSpec, TestSpec};
use crate::test_runner::{TestBackend, TestResult, TestSummary, is_verbose};
use log::info;

/// Run assert, assert_eq, assert_fail, and assert_error against a transaction.
/// Returns `Ok(())` on success, or `Err(message)` on the first failure.
fn run_assertions(tx: &mut Transaction, t: &TestSpec, invariants: &[InvariantSpec]) -> std::result::Result<(), String> {
    for a in &t.asserts {
        if is_verbose() {
            info!("-- assert: {}", a);
        }
        match tx.query(a, &[]) {
            Ok(rows) => match assert_rows_true(&rows) {
                Ok(true) => {}
                Ok(false) => return Err("assert returned false".into()),
                Err(e) => return Err(format!("assert error: {}", e)),
            },
            Err(e) => return Err(format!("assert query error: {}", e)),
        }
    }
    for ae in &t.assert_eq {
        if is_verbose() {
            info!("-- assert-eq: {} == {}", ae.query, ae.expected);
        }
        match tx.query(ae.query.as_str(), &[]) {
            Ok(rows) => match extract_first_column_string(&rows) {
                Ok(actual) => {
                    if actual != ae.expected {
                        return Err(format!(
                            "assert_eq: expected '{}', got '{}'",
                            ae.expected, actual
                        ));
                    }
                }
                Err(e) => return Err(format!("assert_eq error: {}", e)),
            },
            Err(e) => return Err(format!("assert_eq query error: {}", e)),
        }
    }
    for snap in &t.assert_snapshot {
        if is_verbose() {
            info!("-- assert-snapshot: {}", snap.query);
        }
        match tx.query(snap.query.as_str(), &[]) {
            Ok(rows) => {
                if rows.len() != snap.rows.len() {
                    return Err(format!(
                        "assert_snapshot: expected {} rows, got {}",
                        snap.rows.len(),
                        rows.len()
                    ));
                }
                for (ri, (actual_row, expected_row)) in rows.iter().zip(snap.rows.iter()).enumerate() {
                    let num_cols = actual_row.columns().len();
                    if num_cols != expected_row.len() {
                        return Err(format!(
                            "assert_snapshot row {}: expected {} columns, got {}",
                            ri, expected_row.len(), num_cols
                        ));
                    }
                    for (ci, expected_val) in expected_row.iter().enumerate() {
                        match extract_column_string(actual_row, ci) {
                            Ok(actual_val) => {
                                if actual_val != *expected_val {
                                    return Err(format!(
                                        "assert_snapshot row {} col {}: expected '{}', got '{}'",
                                        ri, ci, expected_val, actual_val
                                    ));
                                }
                            }
                            Err(e) => return Err(format!("assert_snapshot row {} col {}: {}", ri, ci, e)),
                        }
                    }
                }
            }
            Err(e) => return Err(format!("assert_snapshot query error: {}", e)),
        }
    }
    for inv in invariants {
        for a in &inv.asserts {
            if is_verbose() {
                info!("-- invariant '{}': {}", inv.name, a);
            }
            match tx.query(a.as_str(), &[]) {
                Ok(rows) => match assert_rows_true(&rows) {
                    Ok(true) => {}
                    Ok(false) => return Err(format!("invariant '{}' returned false", inv.name)),
                    Err(e) => return Err(format!("invariant '{}' error: {}", inv.name, e)),
                },
                Err(e) => return Err(format!("invariant '{}' query error: {}", inv.name, e)),
            }
        }
    }
    for a in &t.assert_fail {
        if is_verbose() {
            info!("-- assert-fail: {}", a);
        }
        match tx.batch_execute(a) {
            Ok(_) => return Err("assert-fail succeeded unexpectedly".into()),
            Err(_) => {}
        }
    }
    for ae in &t.assert_error {
        if is_verbose() {
            info!("-- assert-error: {} (expect: {})", ae.sql, ae.message_contains);
        }
        let mut sp = tx.savepoint("assert_error_sp").map_err(|e| format!("savepoint error: {}", e))?;
        match sp.batch_execute(ae.sql.as_str()) {
            Ok(_) => {
                let _ = sp.rollback();
                return Err(format!(
                    "assert_error: statement succeeded unexpectedly (expected error containing '{}')",
                    ae.message_contains
                ));
            }
            Err(e) => {
                let err_msg = match e.as_db_error() {
                    Some(db) => db.to_string(),
                    None => e.to_string(),
                };
                let _ = sp.rollback();
                if !err_msg.contains(&ae.message_contains) {
                    return Err(format!(
                        "assert_error: error message '{}' does not contain '{}'",
                        err_msg, ae.message_contains
                    ));
                }
            }
        }
    }
    Ok(())
}

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
            let mut failed_msg = String::new();
            let mut ok = true;

            if !t.assert_notify.is_empty() {
                // --- Committed notify path ---
                // Notifications are only delivered after COMMIT, so we cannot
                // use the normal rollback-based isolation for these tests.

                // 1. Open a dedicated listener connection and LISTEN on each channel
                let mut listener = Client::connect(dsn, NoTls)
                    .with_context(|| format!("notify listener: connecting to {}", redacted(dsn)))?;
                for na in &t.assert_notify {
                    let listen_sql = format!("LISTEN {}", na.channel);
                    if is_verbose() {
                        info!("-- listen: {}", listen_sql);
                    }
                    listener.batch_execute(&listen_sql)?;
                }

                // 2. Run setup SQL on main client (auto-committed, no transaction)
                for s in &t.setup {
                    if is_verbose() {
                        info!("-- setup (committed): {}", s);
                    }
                    if let Err(e) = client.batch_execute(s) {
                        failed_msg = format!("setup failed: {}", e);
                        ok = false;
                    }
                }

                // 3. Poll for notifications with a timeout
                //    Collect all available notifications (not just assert_notify.len()),
                //    because earlier setup statements may produce notifications too.
                if ok {
                    let mut received = Vec::new();
                    let mut notifications = listener.notifications();
                    let mut iter = notifications.timeout_iter(Duration::from_secs(2));
                    loop {
                        match iter.next() {
                            Ok(Some(n)) => {
                                if is_verbose() {
                                    info!("-- received notification: channel={}, payload={}", n.channel(), n.payload());
                                }
                                received.push(n);
                            }
                            Ok(None) => break, // timeout — collected everything available
                            Err(e) => {
                                failed_msg = format!("notification poll error: {}", e);
                                ok = false;
                                break;
                            }
                        }
                    }

                    // 4. Verify each assert_notify
                    if ok {
                        for na in &t.assert_notify {
                            let found = received.iter().any(|n| {
                                n.channel() == na.channel
                                    && na.payload_contains.as_ref().map_or(true, |s| n.payload().contains(s.as_str()))
                            });
                            if !found {
                                ok = false;
                                let expected = match &na.payload_contains {
                                    Some(s) => format!("channel='{}' with payload containing '{}'", na.channel, s),
                                    None => format!("channel='{}'", na.channel),
                                };
                                let got: Vec<String> = received
                                    .iter()
                                    .map(|n| format!("{}:{}", n.channel(), n.payload()))
                                    .collect();
                                failed_msg = format!(
                                    "assert_notify: expected {}, got [{}]",
                                    expected,
                                    got.join(", ")
                                );
                                break;
                            }
                        }
                    }
                }

                // 5. Run remaining assertions in a fresh transaction (if any)
                let has_tx_asserts = !t.asserts.is_empty() || !t.assert_eq.is_empty()
                    || !t.assert_fail.is_empty() || !t.assert_error.is_empty()
                    || !t.assert_snapshot.is_empty() || !cfg.invariants.is_empty();
                if ok && has_tx_asserts {
                    let mut tx = client.transaction()?;
                    if let Err(msg) = run_assertions(&mut tx, t, &cfg.invariants) {
                        ok = false;
                        failed_msg = msg;
                    }
                    let _ = tx.rollback();
                }

                // 6. Teardown (cleanup committed data)
                for s in &t.teardown {
                    if is_verbose() {
                        info!("-- teardown: {}", s);
                    }
                    let _ = client.batch_execute(s);
                }
            } else {
                // --- Standard transactional path ---
                let mut tx = client.transaction()?;
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
                    if let Err(msg) = run_assertions(&mut tx, t, &cfg.invariants) {
                        ok = false;
                        failed_msg = msg;
                    }
                }
                let _ = tx.rollback();
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
fn extract_column_string(row: &Row, col: usize) -> Result<String> {
    if let Ok(v) = row.try_get::<usize, String>(col) {
        return Ok(v);
    }
    if let Ok(v) = row.try_get::<usize, bool>(col) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<usize, i64>(col) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<usize, i32>(col) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<usize, f64>(col) {
        return Ok(v.to_string());
    }
    Err(anyhow!("unsupported column type"))
}

fn extract_first_column_string(rows: &[Row]) -> Result<String> {
    if rows.is_empty() {
        return Err(anyhow!("query returned no rows"));
    }
    let row = &rows[0];
    if row.columns().is_empty() {
        return Err(anyhow!("query returned no columns"));
    }
    extract_column_string(row, 0)
}

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
    use super::*;
    use crate::ir::{
        Config, EqAssertSpec, ErrorAssertSpec, InvariantSpec, NotifyAssertSpec,
        SnapshotAssertSpec, TestSpec,
    };
    use crate::test_runner::TestBackend;

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

    // --- Integration tests using testcontainers ---

    fn start_pg() -> (testcontainers_modules::testcontainers::Container<testcontainers_modules::postgres::Postgres>, String) {
        use testcontainers_modules::postgres::Postgres as PgImage;
        use testcontainers_modules::testcontainers::runners::SyncRunner;
        let container = PgImage::default().start().unwrap();
        let host = container.get_host().unwrap();
        let port = container.get_host_port_ipv4(5432).unwrap();
        let dsn = format!("postgres://postgres:postgres@{}:{}/postgres", host, port);
        (container, dsn)
    }

    fn test_spec(name: &str) -> TestSpec {
        TestSpec {
            name: name.into(),
            setup: vec![],
            asserts: vec![],
            assert_fail: vec![],
            assert_notify: vec![],
            assert_eq: vec![],
            assert_error: vec![],
            assert_snapshot: vec![],
            teardown: vec![],
        }
    }

    fn run_one(dsn: &str, test: TestSpec) -> crate::test_runner::TestResult {
        let cfg = Config {
            tests: vec![test],
            ..Default::default()
        };
        let summary = PostgresTestBackend.run(&cfg, dsn, None).unwrap();
        summary.results.into_iter().next().unwrap()
    }

    fn run_one_with_invariants(dsn: &str, test: TestSpec, invariants: Vec<InvariantSpec>) -> crate::test_runner::TestResult {
        let cfg = Config {
            tests: vec![test],
            invariants,
            ..Default::default()
        };
        let summary = PostgresTestBackend.run(&cfg, dsn, None).unwrap();
        summary.results.into_iter().next().unwrap()
    }

    // -- assert --

    #[test]
    fn assert_true_passes() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.asserts = vec!["SELECT 1 = 1".into()];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn assert_false_fails() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.asserts = vec!["SELECT 1 = 2".into()];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("assert returned false"), "msg: {}", r.message);
    }

    // -- assert_eq --

    #[test]
    fn assert_eq_passes() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_eq = vec![EqAssertSpec {
            query: "SELECT 'hello'".into(),
            expected: "hello".into(),
        }];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn assert_eq_fails_on_mismatch() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_eq = vec![EqAssertSpec {
            query: "SELECT 'hello'".into(),
            expected: "world".into(),
        }];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("expected 'world'"), "msg: {}", r.message);
        assert!(r.message.contains("got 'hello'"), "msg: {}", r.message);
    }

    // -- assert_error --

    #[test]
    fn assert_error_passes_on_matching_error() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_error = vec![ErrorAssertSpec {
            sql: "SELECT 1/0".into(),
            message_contains: "division by zero".into(),
        }];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn assert_error_fails_on_wrong_message() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_error = vec![ErrorAssertSpec {
            sql: "SELECT 1/0".into(),
            message_contains: "not found".into(),
        }];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("does not contain 'not found'"), "msg: {}", r.message);
    }

    #[test]
    fn assert_error_fails_when_no_error() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_error = vec![ErrorAssertSpec {
            sql: "SELECT 1".into(),
            message_contains: "anything".into(),
        }];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("succeeded unexpectedly"), "msg: {}", r.message);
    }

    // -- assert_fail --

    #[test]
    fn assert_fail_passes_on_error() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_fail = vec!["SELECT 1 FROM nonexistent_table_xyz".into()];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn assert_fail_fails_on_success() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_fail = vec!["SELECT 1".into()];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("succeeded unexpectedly"), "msg: {}", r.message);
    }

    // -- assert_snapshot --

    #[test]
    fn assert_snapshot_passes() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_snapshot = vec![SnapshotAssertSpec {
            query: "SELECT 1 AS a, 'x' AS b".into(),
            rows: vec![vec!["1".into(), "x".into()]],
        }];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn assert_snapshot_fails_on_wrong_value() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_snapshot = vec![SnapshotAssertSpec {
            query: "SELECT 1 AS a, 'x' AS b".into(),
            rows: vec![vec!["1".into(), "y".into()]],
        }];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("expected 'y'"), "msg: {}", r.message);
        assert!(r.message.contains("got 'x'"), "msg: {}", r.message);
    }

    #[test]
    fn assert_snapshot_fails_on_wrong_row_count() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_snapshot = vec![SnapshotAssertSpec {
            query: "SELECT 1".into(),
            rows: vec![vec!["1".into()], vec!["2".into()]],
        }];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("expected 2 rows, got 1"), "msg: {}", r.message);
    }

    // -- invariants --

    #[test]
    fn invariant_passes() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.asserts = vec!["SELECT true".into()];
        let inv = InvariantSpec {
            name: "always_true".into(),
            asserts: vec!["SELECT 1 = 1".into()],
        };
        let r = run_one_with_invariants(&dsn, t, vec![inv]);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn invariant_failure_fails_test() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.asserts = vec!["SELECT true".into()];
        let inv = InvariantSpec {
            name: "always_false".into(),
            asserts: vec!["SELECT 1 = 2".into()],
        };
        let r = run_one_with_invariants(&dsn, t, vec![inv]);
        assert!(!r.passed);
        assert!(r.message.contains("invariant 'always_false'"), "msg: {}", r.message);
    }

    // -- assert_notify --

    #[test]
    fn assert_notify_passes() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.setup = vec!["SELECT pg_notify('test_ch', 'hello')".into()];
        t.assert_notify = vec![NotifyAssertSpec {
            channel: "test_ch".into(),
            payload_contains: Some("hello".into()),
        }];
        t.teardown = vec![];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    // -- setup failure --

    #[test]
    fn setup_failure_fails_test() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.setup = vec!["SELECT 1 FROM nonexistent_table_xyz".into()];
        t.asserts = vec!["SELECT true".into()];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("setup failed"), "msg: {}", r.message);
    }

    // -- multiple assertions in one test --

    #[test]
    fn multiple_assertions_all_pass() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.asserts = vec!["SELECT true".into()];
        t.assert_eq = vec![EqAssertSpec {
            query: "SELECT '42'".into(),
            expected: "42".into(),
        }];
        t.assert_snapshot = vec![SnapshotAssertSpec {
            query: "SELECT 1 AS n".into(),
            rows: vec![vec!["1".into()]],
        }];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    // -- only filtering --

    fn run_cfg(dsn: &str, cfg: Config, only: Option<&HashSet<String>>) -> crate::test_runner::TestSummary {
        PostgresTestBackend.run(&cfg, dsn, only).unwrap()
    }

    #[test]
    fn only_filter_runs_selected_test() {
        let (_c, dsn) = start_pg();
        let mut t1 = test_spec("included");
        t1.asserts = vec!["SELECT true".into()];
        let mut t2 = test_spec("excluded");
        t2.asserts = vec!["SELECT true".into()];
        let cfg = Config {
            tests: vec![t1, t2],
            ..Default::default()
        };
        let only: HashSet<String> = ["included".to_string()].into();
        let summary = run_cfg(&dsn, cfg, Some(&only));
        assert_eq!(summary.total, 1);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.results[0].name, "included");
    }

    #[test]
    fn only_filter_no_match_returns_empty() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("some_test");
        t.asserts = vec!["SELECT true".into()];
        let cfg = Config {
            tests: vec![t],
            ..Default::default()
        };
        let only: HashSet<String> = ["nonexistent".to_string()].into();
        let summary = run_cfg(&dsn, cfg, Some(&only));
        assert_eq!(summary.total, 0);
        assert_eq!(summary.passed, 0);
        assert_eq!(summary.failed, 0);
    }

    // -- mixed assert_notify + transactional assertions --

    #[test]
    fn notify_with_transactional_assertions() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.setup = vec!["SELECT pg_notify('ch', 'payload')".into()];
        t.assert_notify = vec![NotifyAssertSpec {
            channel: "ch".into(),
            payload_contains: Some("payload".into()),
        }];
        t.asserts = vec!["SELECT 1 = 1".into()];
        t.assert_eq = vec![EqAssertSpec {
            query: "SELECT 'ok'".into(),
            expected: "ok".into(),
        }];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn notify_with_failing_transactional_assertion() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.setup = vec!["SELECT pg_notify('ch', 'payload')".into()];
        t.assert_notify = vec![NotifyAssertSpec {
            channel: "ch".into(),
            payload_contains: Some("payload".into()),
        }];
        t.asserts = vec!["SELECT 1 = 2".into()]; // will fail
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("assert returned false"), "msg: {}", r.message);
    }

    // -- assert_notify edge cases --

    #[test]
    fn assert_notify_without_payload_passes() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.setup = vec!["SELECT pg_notify('ch', 'anything')".into()];
        t.assert_notify = vec![NotifyAssertSpec {
            channel: "ch".into(),
            payload_contains: None,
        }];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn assert_notify_wrong_channel_fails() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.setup = vec!["SELECT pg_notify('actual_ch', 'data')".into()];
        t.assert_notify = vec![NotifyAssertSpec {
            channel: "wrong_ch".into(),
            payload_contains: None,
        }];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("assert_notify"), "msg: {}", r.message);
    }

    #[test]
    fn assert_notify_wrong_payload_fails() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.setup = vec!["SELECT pg_notify('ch', 'actual')".into()];
        t.assert_notify = vec![NotifyAssertSpec {
            channel: "ch".into(),
            payload_contains: Some("expected".into()),
        }];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("assert_notify"), "msg: {}", r.message);
        assert!(r.message.contains("expected"), "msg: {}", r.message);
    }

    #[test]
    fn assert_notify_multiple_channels() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.setup = vec![
            "SELECT pg_notify('ch1', 'one')".into(),
            "SELECT pg_notify('ch2', 'two')".into(),
        ];
        t.assert_notify = vec![
            NotifyAssertSpec {
                channel: "ch1".into(),
                payload_contains: Some("one".into()),
            },
            NotifyAssertSpec {
                channel: "ch2".into(),
                payload_contains: Some("two".into()),
            },
        ];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn setup_failure_on_notify_path() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.setup = vec!["SELECT 1 FROM nonexistent_xyz".into()];
        t.assert_notify = vec![NotifyAssertSpec {
            channel: "ch".into(),
            payload_contains: None,
        }];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("setup failed"), "msg: {}", r.message);
    }

    // -- multiple tests with mixed pass/fail summary --

    #[test]
    fn summary_counts_mixed_pass_fail() {
        let (_c, dsn) = start_pg();
        let mut t1 = test_spec("pass1");
        t1.asserts = vec!["SELECT true".into()];
        let mut t2 = test_spec("fail1");
        t2.asserts = vec!["SELECT false".into()];
        let mut t3 = test_spec("pass2");
        t3.asserts = vec!["SELECT 1 = 1".into()];
        let cfg = Config {
            tests: vec![t1, t2, t3],
            ..Default::default()
        };
        let summary = run_cfg(&dsn, cfg, None);
        assert_eq!(summary.total, 3);
        assert_eq!(summary.passed, 2);
        assert_eq!(summary.failed, 1);
        assert!(summary.results[0].passed);
        assert!(!summary.results[1].passed);
        assert!(summary.results[2].passed);
    }

    #[test]
    fn summary_results_preserve_order_and_names() {
        let (_c, dsn) = start_pg();
        let mut t1 = test_spec("alpha");
        t1.asserts = vec!["SELECT true".into()];
        let mut t2 = test_spec("beta");
        t2.asserts = vec!["SELECT true".into()];
        let mut t3 = test_spec("gamma");
        t3.asserts = vec!["SELECT true".into()];
        let cfg = Config {
            tests: vec![t1, t2, t3],
            ..Default::default()
        };
        let summary = run_cfg(&dsn, cfg, None);
        let names: Vec<&str> = summary.results.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "beta", "gamma"]);
    }

    // -- temporary database setup/cleanup --

    #[test]
    fn temporary_database_setup_and_cleanup() {
        let (_c, dsn) = start_pg();
        let db_name = "test_temp_db_integration";
        let new_dsn = PostgresTestBackend
            .setup_temporary_database(&dsn, db_name, false)
            .expect("setup_temporary_database failed");
        assert!(new_dsn.contains(db_name), "DSN should contain db name: {}", new_dsn);

        // Verify we can connect to the new database
        let client = Client::connect(&new_dsn, NoTls);
        assert!(client.is_ok(), "should connect to temp database");
        drop(client);

        // Cleanup
        PostgresTestBackend
            .cleanup_temporary_database(&dsn, db_name, false)
            .expect("cleanup failed");

        // Verify connection to cleaned-up database fails
        let client = Client::connect(&new_dsn, NoTls);
        assert!(client.is_err(), "database should be dropped");
    }

    #[test]
    fn temporary_database_setup_replaces_existing() {
        let (_c, dsn) = start_pg();
        let db_name = "test_temp_replace_db";
        // Create once
        let dsn1 = PostgresTestBackend
            .setup_temporary_database(&dsn, db_name, false)
            .expect("first setup failed");
        // Create table in the first database
        let mut c1 = Client::connect(&dsn1, NoTls).unwrap();
        c1.batch_execute("CREATE TABLE marker (id int)").unwrap();
        drop(c1);

        // Setup again — should DROP and recreate, losing the table
        let dsn2 = PostgresTestBackend
            .setup_temporary_database(&dsn, db_name, false)
            .expect("second setup failed");
        let mut c2 = Client::connect(&dsn2, NoTls).unwrap();
        let result = c2.batch_execute("SELECT 1 FROM marker");
        assert!(result.is_err(), "table should not exist after re-setup");
        drop(c2);

        PostgresTestBackend
            .cleanup_temporary_database(&dsn, db_name, false)
            .unwrap();
    }

    #[test]
    fn cleanup_nonexistent_database_is_ok() {
        let (_c, dsn) = start_pg();
        let result = PostgresTestBackend
            .cleanup_temporary_database(&dsn, "db_that_does_not_exist", false);
        assert!(result.is_ok(), "cleanup of nonexistent db should succeed");
    }

    #[test]
    fn supports_temporary_database_is_true() {
        assert!(PostgresTestBackend.supports_temporary_database());
    }

    // -- assert_snapshot edge cases --

    #[test]
    fn assert_snapshot_column_count_mismatch() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_snapshot = vec![SnapshotAssertSpec {
            query: "SELECT 1 AS a, 2 AS b, 3 AS c".into(),
            rows: vec![vec!["1".into(), "2".into()]], // expect 2 cols, query returns 3
        }];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("expected 2 columns, got 3"), "msg: {}", r.message);
    }

    #[test]
    fn assert_snapshot_multi_row_passes() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_snapshot = vec![SnapshotAssertSpec {
            query: "SELECT * FROM (VALUES (1, 'a'), (2, 'b'), (3, 'c')) AS t(id, name)".into(),
            rows: vec![
                vec!["1".into(), "a".into()],
                vec!["2".into(), "b".into()],
                vec!["3".into(), "c".into()],
            ],
        }];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn assert_snapshot_multi_row_second_row_mismatch() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_snapshot = vec![SnapshotAssertSpec {
            query: "SELECT * FROM (VALUES (1, 'a'), (2, 'b')) AS t(id, name)".into(),
            rows: vec![
                vec!["1".into(), "a".into()],
                vec!["2".into(), "WRONG".into()],
            ],
        }];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("row 1"), "msg: {}", r.message);
        assert!(r.message.contains("expected 'WRONG'"), "msg: {}", r.message);
        assert!(r.message.contains("got 'b'"), "msg: {}", r.message);
    }

    // -- multiple invariants and assertion ordering --

    #[test]
    fn multiple_invariants_all_pass() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.asserts = vec!["SELECT true".into()];
        let inv1 = InvariantSpec {
            name: "inv1".into(),
            asserts: vec!["SELECT 1 = 1".into()],
        };
        let inv2 = InvariantSpec {
            name: "inv2".into(),
            asserts: vec!["SELECT 2 = 2".into()],
        };
        let r = run_one_with_invariants(&dsn, t, vec![inv1, inv2]);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn multiple_invariants_second_fails() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.asserts = vec!["SELECT true".into()];
        let inv1 = InvariantSpec {
            name: "ok_inv".into(),
            asserts: vec!["SELECT 1 = 1".into()],
        };
        let inv2 = InvariantSpec {
            name: "bad_inv".into(),
            asserts: vec!["SELECT 1 = 2".into()],
        };
        let r = run_one_with_invariants(&dsn, t, vec![inv1, inv2]);
        assert!(!r.passed);
        assert!(r.message.contains("invariant 'bad_inv'"), "msg: {}", r.message);
    }

    #[test]
    fn invariant_with_multiple_asserts_second_fails() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.asserts = vec!["SELECT true".into()];
        let inv = InvariantSpec {
            name: "multi_assert".into(),
            asserts: vec!["SELECT 1 = 1".into(), "SELECT 1 = 2".into()],
        };
        let r = run_one_with_invariants(&dsn, t, vec![inv]);
        assert!(!r.passed);
        assert!(r.message.contains("invariant 'multi_assert'"), "msg: {}", r.message);
    }

    #[test]
    fn assert_passes_but_assert_eq_fails() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.asserts = vec!["SELECT true".into()];
        t.assert_eq = vec![EqAssertSpec {
            query: "SELECT 'a'".into(),
            expected: "b".into(),
        }];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("assert_eq"), "msg: {}", r.message);
    }

    #[test]
    fn assert_fails_skips_later_assertions() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.asserts = vec!["SELECT 1 = 2".into()]; // fails
        t.assert_eq = vec![EqAssertSpec {
            query: "SELECT 'a'".into(),
            expected: "b".into(), // would also fail, but shouldn't be reached
        }];
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        // The error should be from assert, not assert_eq
        assert!(r.message.contains("assert returned false"), "msg: {}", r.message);
    }

    // -- type conversion edge cases --

    #[test]
    fn assert_with_integer_truthiness() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.asserts = vec!["SELECT 42".into()]; // non-zero integer is truthy
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn assert_with_zero_is_false() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.asserts = vec!["SELECT 0".into()]; // zero is falsy
        let r = run_one(&dsn, t);
        assert!(!r.passed);
        assert!(r.message.contains("assert returned false"), "msg: {}", r.message);
    }

    #[test]
    fn assert_snapshot_with_boolean() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_snapshot = vec![SnapshotAssertSpec {
            query: "SELECT true AS b".into(),
            rows: vec![vec!["true".into()]],
        }];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn assert_snapshot_with_float() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_snapshot = vec![SnapshotAssertSpec {
            query: "SELECT 3.14::float8 AS f".into(),
            rows: vec![vec!["3.14".into()]],
        }];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn assert_eq_with_integer() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_eq = vec![EqAssertSpec {
            query: "SELECT 100".into(),
            expected: "100".into(),
        }];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }

    #[test]
    fn assert_eq_with_boolean() {
        let (_c, dsn) = start_pg();
        let mut t = test_spec("t");
        t.assert_eq = vec![EqAssertSpec {
            query: "SELECT true".into(),
            expected: "true".into(),
        }];
        let r = run_one(&dsn, t);
        assert!(r.passed, "expected pass: {}", r.message);
    }
}
