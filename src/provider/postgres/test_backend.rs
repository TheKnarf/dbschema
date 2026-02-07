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
        // Run scenarios if the scenario feature is enabled
        #[cfg(feature = "scenario")]
        {
            if !cfg.scenarios.is_empty() {
                let (scenario_results, scenario_stats) = crate::scenario::run_scenarios(cfg, &mut client)
                    .context("running scenarios")?;
                for r in scenario_results {
                    if r.passed {
                        passed += 1;
                    }
                    results.push(r);
                }
                for s in &scenario_stats {
                    info!(
                        "Scenario '{}': seed={}, {} answer sets, {} passed, {} failed",
                        s.scenario_name, s.seed, s.models_tested, s.passed, s.failed
                    );
                }
            }
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
    #[cfg(feature = "scenario")]
    use crate::ir::{ScenarioMapSpec, ScenarioSpec, ScenarioStepSpec};
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

    // -- scenario integration --

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_detects_trigger_bug() {
        let (_c, dsn) = start_pg();

        // Apply the buggy schema via a separate connection
        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE orders (id serial PRIMARY KEY, max_weight int NOT NULL);
                CREATE TABLE order_items (
                    id serial PRIMARY KEY,
                    order_id int NOT NULL REFERENCES orders(id),
                    product text NOT NULL,
                    weight int NOT NULL
                );

                -- BUG: SUM doesn't include NEW.weight (the row being inserted)
                CREATE FUNCTION check_order_weight() RETURNS trigger AS $$
                DECLARE current_total int; max_w int;
                BEGIN
                    SELECT COALESCE(SUM(weight), 0) INTO current_total
                        FROM order_items WHERE order_id = NEW.order_id;
                    SELECT max_weight INTO max_w FROM orders WHERE id = NEW.order_id;
                    IF current_total > max_w THEN
                        RAISE EXCEPTION 'Order weight limit exceeded';
                    END IF;
                    RETURN NEW;
                END;
                $$ LANGUAGE plpgsql;

                CREATE TRIGGER trg_check_weight
                    BEFORE INSERT ON order_items
                    FOR EACH ROW EXECUTE FUNCTION check_order_weight();
            ").unwrap();
        }

        // Manual test: single heavy item (weight=80, limit=100) → passes due to the bug
        let manual_test = TestSpec {
            name: "manual_single_item".into(),
            setup: vec![
                "INSERT INTO orders (max_weight) VALUES (100)".into(),
                "INSERT INTO order_items (order_id, product, weight) VALUES (currval('orders_id_seq'), 'heavy', 80)".into(),
            ],
            asserts: vec![
                // Invariant check inline: total weight should be <= max_weight
                // This passes because 80 <= 100
                "SELECT (SELECT COALESCE(SUM(weight),0) FROM order_items WHERE order_id = o.id) <= o.max_weight FROM orders o WHERE id = currval('orders_id_seq')".into(),
            ],
            assert_fail: vec![],
            assert_notify: vec![],
            assert_eq: vec![],
            assert_error: vec![],
            assert_snapshot: vec![],
            teardown: vec![],
        };

        // Invariant: total weight per order must not exceed max_weight
        let invariant = InvariantSpec {
            name: "weight_within_limit".into(),
            asserts: vec![
                "SELECT NOT EXISTS (SELECT 1 FROM orders o WHERE (SELECT COALESCE(SUM(weight),0) FROM order_items WHERE order_id = o.id) > o.max_weight)".into(),
            ],
        };

        // Scenario: clingo generates diverse item combinations
        let scenario = ScenarioSpec {
            name: "order_weight".into(),
            program: "item(a;b;c). weight(30;40;60;80). 1 { add(I,W) : item(I), weight(W) } 3. :- add(I,W1), add(I,W2), W1 != W2.".into(),
            setup: vec![
                "INSERT INTO orders (max_weight) VALUES (100)".into(),
            ],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "add".into(),
                    sql: "INSERT INTO order_items (order_id, product, weight) VALUES (currval('orders_id_seq'), '{1}', {2})".into(),
                    order_by: None,
                },
            ],
            runs: 0, // all answer sets
            checks: vec![],
            expect_error: false,
            assert_eq: vec![],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![],
            seed: None,
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            tests: vec![manual_test],
            invariants: vec![invariant],
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();

        // The manual test should pass (proves manual testing misses the bug)
        let manual_result = summary.results.iter().find(|r| r.name == "manual_single_item").unwrap();
        assert!(manual_result.passed, "manual test should pass: {}", manual_result.message);

        // Scenario should have generated multiple test cases
        let scenario_results: Vec<_> = summary.results.iter()
            .filter(|r| r.name.starts_with("order_weight["))
            .collect();
        assert!(scenario_results.len() > 1, "scenario should generate multiple test cases, got {}", scenario_results.len());

        // Some scenario tests should fail (multi-item combos exceeding 100)
        let failed: Vec<_> = scenario_results.iter().filter(|r| !r.passed).collect();
        assert!(!failed.is_empty(), "some scenario tests should fail (bug detected)");

        // Some scenario tests should pass (single items or light combos ≤ 100)
        let passed: Vec<_> = scenario_results.iter().filter(|r| r.passed).collect();
        assert!(!passed.is_empty(), "some scenario tests should pass (light combos)");

        // Failures come in two flavors due to the off-by-one bug:
        // 1. Invariant violations: the trigger allowed inserts that shouldn't have been
        //    (e.g. two items of 60 each → total 120 > 100, but trigger only checked prior sum)
        // 2. Trigger exceptions: when prior items already exceed the limit, the trigger
        //    catches a later insert (e.g. after two 60s total 120, a third 60 triggers the check)
        // The key point is that SOME failures are invariant violations — proving the bug.
        let invariant_failures: Vec<_> = failed.iter()
            .filter(|r| r.message.contains("weight_within_limit"))
            .collect();
        assert!(
            !invariant_failures.is_empty(),
            "some failures should be invariant violations (the actual bug), got messages: {:?}",
            failed.iter().map(|r| &r.message).collect::<Vec<_>>()
        );
    }

    // -- #5: order_by on map blocks --

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_order_by_sorts_by_argument() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE ordered_log (id serial PRIMARY KEY, item text NOT NULL, position int NOT NULL);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "ordered_insert".into(),
            program: "add(c,3). add(a,1). add(b,2).".into(),
            setup: vec![],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "add".into(),
                    sql: "INSERT INTO ordered_log (item, position) VALUES ('{1}', {2})".into(),
                    order_by: Some(2), // sort by position (2nd arg, numeric)
                },
            ],
            runs: 0,
            checks: vec![],
            expect_error: false,
            assert_eq: vec![],
            assert_snapshot: vec![
                SnapshotAssertSpec {
                    query: "SELECT item, position::text FROM ordered_log ORDER BY id".into(),
                    rows: vec![
                        vec!["a".into(), "1".into()],
                        vec!["b".into(), "2".into()],
                        vec!["c".into(), "3".into()],
                    ],
                },
            ],
            params: vec![],
            teardown: vec![],
            seed: None,
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        assert!(summary.results[0].passed, "order_by test failed: {}", summary.results[0].message);
    }

    // -- #6: check blocks (scenario-scoped invariants) --

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_check_blocks_pass() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE items (id serial PRIMARY KEY, val int NOT NULL);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "with_checks".into(),
            program: "add(1). add(2).".into(),
            setup: vec![],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "add".into(),
                    sql: "INSERT INTO items (val) VALUES ({1})".into(),
                    order_by: None,
                },
            ],
            runs: 0,
            checks: vec![
                InvariantSpec {
                    name: "positive_vals".into(),
                    asserts: vec!["SELECT NOT EXISTS (SELECT 1 FROM items WHERE val <= 0)".into()],
                },
            ],
            expect_error: false,
            assert_eq: vec![],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![],
            seed: None,
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        assert!(summary.results[0].passed, "check block test failed: {}", summary.results[0].message);
    }

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_check_blocks_fail() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE items (id serial PRIMARY KEY, val int NOT NULL);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "failing_check".into(),
            program: "add(1). add(-5).".into(),
            setup: vec![],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "add".into(),
                    sql: "INSERT INTO items (val) VALUES ({1})".into(),
                    order_by: None,
                },
            ],
            runs: 0,
            checks: vec![
                InvariantSpec {
                    name: "positive_vals".into(),
                    asserts: vec!["SELECT NOT EXISTS (SELECT 1 FROM items WHERE val <= 0)".into()],
                },
            ],
            expect_error: false,
            assert_eq: vec![],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![],
            seed: None,
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        assert!(!summary.results[0].passed, "check should have failed");
        assert!(summary.results[0].message.contains("positive_vals"), "failure should mention the check name: {}", summary.results[0].message);
    }

    // -- #7: expect_error --

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_expect_error_passes_on_sql_error() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE strict_table (id serial PRIMARY KEY, name text NOT NULL UNIQUE);
            ").unwrap();
        }

        // Two maps that both insert into the same UNIQUE column → second one fails
        let scenario = ScenarioSpec {
            name: "expected_error".into(),
            program: "first(alice). second(alice).".into(),
            setup: vec![],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "first".into(),
                    sql: "INSERT INTO strict_table (name) VALUES ('{1}')".into(),
                    order_by: None,
                },
                ScenarioMapSpec {
                    atom_name: "second".into(),
                    sql: "INSERT INTO strict_table (name) VALUES ('{1}')".into(),
                    order_by: None,
                },
            ],
            runs: 0,
            checks: vec![],
            expect_error: true,
            assert_eq: vec![],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![],
            seed: None,
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        // Should pass because we expect the error
        assert!(summary.results[0].passed, "expect_error test should pass: {}", summary.results[0].message);
        assert!(summary.results[0].message.contains("expected error"), "message should indicate expected error: {}", summary.results[0].message);
    }

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_expect_error_false_fails_on_sql_error() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE strict_table2 (id serial PRIMARY KEY, name text NOT NULL UNIQUE);
            ").unwrap();
        }

        // Two maps that both insert into the same UNIQUE column → second one fails
        let scenario = ScenarioSpec {
            name: "unexpected_error".into(),
            program: "first(alice). second(alice).".into(),
            setup: vec![],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "first".into(),
                    sql: "INSERT INTO strict_table2 (name) VALUES ('{1}')".into(),
                    order_by: None,
                },
                ScenarioMapSpec {
                    atom_name: "second".into(),
                    sql: "INSERT INTO strict_table2 (name) VALUES ('{1}')".into(),
                    order_by: None,
                },
            ],
            runs: 0,
            checks: vec![],
            expect_error: false, // not expecting errors
            assert_eq: vec![],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![],
            seed: None,
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        // Should fail because we don't expect the error
        assert!(!summary.results[0].passed, "should fail when error is unexpected");
    }

    // -- #8: assert_eq and assert_snapshot inside scenarios --

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_assert_eq_passes() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE counters (id serial PRIMARY KEY, val int NOT NULL DEFAULT 0);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "eq_check".into(),
            program: "inc(1). inc(2).".into(),
            setup: vec![
                "INSERT INTO counters (val) VALUES (0)".into(),
            ],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "inc".into(),
                    sql: "UPDATE counters SET val = val + {1} WHERE id = currval('counters_id_seq')".into(),
                    order_by: None,
                },
            ],
            runs: 0,
            checks: vec![],
            expect_error: false,
            assert_eq: vec![
                EqAssertSpec {
                    query: "SELECT val::text FROM counters WHERE id = currval('counters_id_seq')".into(),
                    expected: "3".into(), // 0 + 1 + 2
                },
            ],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![],
            seed: None,
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        assert!(summary.results[0].passed, "assert_eq should pass: {}", summary.results[0].message);
    }

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_assert_eq_fails_on_mismatch() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE counters2 (id serial PRIMARY KEY, val int NOT NULL DEFAULT 0);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "eq_fail".into(),
            program: "inc(1).".into(),
            setup: vec![
                "INSERT INTO counters2 (val) VALUES (0)".into(),
            ],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "inc".into(),
                    sql: "UPDATE counters2 SET val = val + {1} WHERE id = currval('counters2_id_seq')".into(),
                    order_by: None,
                },
            ],
            runs: 0,
            checks: vec![],
            expect_error: false,
            assert_eq: vec![
                EqAssertSpec {
                    query: "SELECT val::text FROM counters2 WHERE id = currval('counters2_id_seq')".into(),
                    expected: "999".into(), // wrong
                },
            ],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![],
            seed: None,
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        assert!(!summary.results[0].passed, "assert_eq should fail on mismatch");
        assert!(summary.results[0].message.contains("assert_eq failed"), "message: {}", summary.results[0].message);
    }

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_assert_snapshot_passes() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE log_entries (id serial PRIMARY KEY, msg text NOT NULL);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "snap_check".into(),
            program: "log(hello). log(world).".into(),
            setup: vec![],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "log".into(),
                    sql: "INSERT INTO log_entries (msg) VALUES ('{1}')".into(),
                    order_by: None,
                },
            ],
            runs: 0,
            checks: vec![],
            expect_error: false,
            assert_eq: vec![],
            assert_snapshot: vec![
                SnapshotAssertSpec {
                    query: "SELECT msg FROM log_entries ORDER BY msg".into(),
                    rows: vec![
                        vec!["hello".into()],
                        vec!["world".into()],
                    ],
                },
            ],
            params: vec![],
            teardown: vec![],
            seed: None,
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        assert!(summary.results[0].passed, "assert_snapshot should pass: {}", summary.results[0].message);
    }

    // -- #9: answer set labeling --

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_test_name_includes_atoms() {
        let (_c, dsn) = start_pg();

        let scenario = ScenarioSpec {
            name: "labeled".into(),
            program: "fact(a). fact(b).".into(),
            setup: vec![],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "fact".into(),
                    sql: "SELECT 1".into(),
                    order_by: None,
                },
            ],
            runs: 1, // just one answer set
            checks: vec![],
            expect_error: false,
            assert_eq: vec![],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![],
            seed: None,
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        let name = &summary.results[0].name;
        // Test name should contain atoms like "labeled[0: fact(a), fact(b)]"
        assert!(name.starts_with("labeled[0:"), "test name should include index: {}", name);
        assert!(name.contains("fact"), "test name should include atom names: {}", name);
    }

    // -- #10: params (#const injection) --

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_params_inject_const() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE param_log (id serial PRIMARY KEY, val int NOT NULL);
            ").unwrap();
        }

        // Use #const to limit the range
        let scenario = ScenarioSpec {
            name: "parameterized".into(),
            program: "val(1..n). { pick(V) : val(V) }.".into(),
            setup: vec![],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "pick".into(),
                    sql: "INSERT INTO param_log (val) VALUES ({1})".into(),
                    order_by: None,
                },
            ],
            runs: 3, // limit to 3 answer sets
            checks: vec![
                InvariantSpec {
                    name: "vals_in_range".into(),
                    asserts: vec!["SELECT NOT EXISTS (SELECT 1 FROM param_log WHERE val < 1 OR val > 2)".into()],
                },
            ],
            expect_error: false,
            assert_eq: vec![],
            assert_snapshot: vec![],
            params: vec![
                ("n".into(), "2".into()), // #const n=2
            ],
            teardown: vec![],
            seed: None,
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert!(summary.results.len() <= 3, "should have at most 3 answer sets");
        // All should pass since vals are limited to 1..2
        for r in &summary.results {
            assert!(r.passed, "parameterized test should pass: {}", r.message);
        }
    }

    // -- #11: teardown --

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_teardown_runs_after_rollback() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE teardown_marker (id serial PRIMARY KEY, note text NOT NULL);
                CREATE TABLE teardown_log (id serial PRIMARY KEY, cleaned boolean NOT NULL DEFAULT false);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "with_teardown".into(),
            program: "act(1).".into(),
            setup: vec![
                "INSERT INTO teardown_marker (note) VALUES ('test')".into(),
            ],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "act".into(),
                    sql: "SELECT {1}".into(),
                    order_by: None,
                },
            ],
            runs: 0,
            checks: vec![],
            expect_error: false,
            assert_eq: vec![],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![
                "INSERT INTO teardown_log (cleaned) VALUES (true)".into(),
            ],
            seed: None,
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        assert!(summary.results[0].passed, "teardown test should pass: {}", summary.results[0].message);

        // Verify teardown ran (it runs outside the transaction, so it should be committed)
        let mut verify_client = Client::connect(&dsn, NoTls).unwrap();
        let rows = verify_client.query("SELECT COUNT(*) FROM teardown_log WHERE cleaned = true", &[]).unwrap();
        let count: i64 = rows[0].get(0);
        assert!(count >= 1, "teardown should have inserted into teardown_log");

        // Setup was rolled back, so teardown_marker should be empty
        let rows = verify_client.query("SELECT COUNT(*) FROM teardown_marker", &[]).unwrap();
        let count: i64 = rows[0].get(0);
        assert_eq!(count, 0, "setup should have been rolled back");
    }

    // -- combined features --

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_all_features_combined() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE products (id serial PRIMARY KEY, name text NOT NULL, price int NOT NULL);
                CREATE TABLE combined_teardown (id serial PRIMARY KEY, done boolean NOT NULL);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "combined".into(),
            // Use params to set max price
            program: "product(apple,10). product(banana,20). { buy(P,C) : product(P,C) }.".into(),
            setup: vec![],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "buy".into(),
                    sql: "INSERT INTO products (name, price) VALUES ('{1}', {2})".into(),
                    order_by: Some(2), // order by price
                },
            ],
            runs: 3, // limit answer sets
            checks: vec![
                InvariantSpec {
                    name: "no_negative_prices".into(),
                    asserts: vec!["SELECT NOT EXISTS (SELECT 1 FROM products WHERE price < 0)".into()],
                },
            ],
            expect_error: false,
            assert_eq: vec![],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![
                "INSERT INTO combined_teardown (done) VALUES (true)".into(),
            ],
            seed: None,
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert!(summary.results.len() <= 3, "should have at most 3 answer sets");
        for r in &summary.results {
            assert!(r.passed, "combined test should pass: {}", r.message);
            // Check answer set labeling
            assert!(r.name.starts_with("combined["), "test name should include scenario name: {}", r.name);
        }

        // Verify teardown ran for each answer set
        let mut verify_client = Client::connect(&dsn, NoTls).unwrap();
        let rows = verify_client.query("SELECT COUNT(*) FROM combined_teardown WHERE done = true", &[]).unwrap();
        let count: i64 = rows[0].get(0);
        assert!(count >= 1, "teardown should have run at least once");
    }

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_with_explicit_seed() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE seed_items (id serial PRIMARY KEY, name text);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "seeded".into(),
            program: "item(a;b;c). 1 { pick(I) : item(I) } 2.".into(),
            setup: vec![],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "pick".into(),
                    sql: "INSERT INTO seed_items (name) VALUES ('{1}')".into(),
                    order_by: None,
                },
            ],
            runs: 3,
            checks: vec![],
            expect_error: false,
            assert_eq: vec![],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![],
            seed: Some(42),
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 3);
        assert_eq!(summary.passed, 3, "all seeded scenario tests should pass");
    }

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_seed_in_failure_message() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE fail_items (id serial PRIMARY KEY, name text NOT NULL UNIQUE);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "seed_fail".into(),
            // Generate answer sets where two atoms produce the same insert, causing a unique violation
            program: "item(a). 1 { dup(I) : item(I) } 1. :- not dup(a).".into(),
            setup: vec![
                "INSERT INTO fail_items (name) VALUES ('a')".into(),
            ],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "dup".into(),
                    sql: "INSERT INTO fail_items (name) VALUES ('{1}')".into(),
                    order_by: None,
                },
            ],
            runs: 1,
            checks: vec![],
            expect_error: false,
            assert_eq: vec![],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![],
            seed: Some(99),
            steps: None,
            step_blocks: vec![],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.failed, 1, "should have one failure");
        let failed = summary.results.iter().find(|r| !r.passed).unwrap();
        assert!(
            failed.message.contains("(seed: 99)"),
            "failure message should contain seed: {}",
            failed.message
        );
    }

    // -- multi-step scenarios --

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_multi_step_basic() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE step_items (id serial PRIMARY KEY, name text NOT NULL);
                CREATE TABLE step_log (id serial PRIMARY KEY, item_name text NOT NULL, action text NOT NULL);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "multi_step_basic".into(),
            program: "#program base.\nitem(apple; banana).\n#program step(t).\nadd(I) :- item(I), t == 1.\nlog(I) :- item(I), t == 2.".into(),
            setup: vec![],
            maps: vec![],
            runs: 0,
            checks: vec![],
            expect_error: false,
            assert_eq: vec![
                EqAssertSpec {
                    query: "SELECT COUNT(*)::text FROM step_items".into(),
                    expected: "2".into(),
                },
            ],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![
                "DELETE FROM step_log".into(),
                "DELETE FROM step_items".into(),
            ],
            seed: Some(1),
            steps: Some(2),
            step_blocks: vec![
                ScenarioStepSpec {
                    setup: vec![],
                    maps: vec![
                        ScenarioMapSpec {
                            atom_name: "add".into(),
                            sql: "INSERT INTO step_items (name) VALUES ('{1}')".into(),
                            order_by: None,
                        },
                    ],
                    checks: vec![],
                    assert_eq: vec![],
                    assert_snapshot: vec![],
                },
                ScenarioStepSpec {
                    setup: vec![],
                    maps: vec![
                        ScenarioMapSpec {
                            atom_name: "log".into(),
                            sql: "INSERT INTO step_log (item_name, action) VALUES ('{1}', 'logged')".into(),
                            order_by: None,
                        },
                    ],
                    checks: vec![],
                    assert_eq: vec![
                        EqAssertSpec {
                            query: "SELECT COUNT(*)::text FROM step_log".into(),
                            expected: "2".into(),
                        },
                    ],
                    assert_snapshot: vec![],
                },
            ],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        assert!(summary.results[0].passed, "multi-step basic test should pass: {}", summary.results[0].message);
    }

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_multi_step_data_accumulates() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE accum (id serial PRIMARY KEY, val int NOT NULL);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "accum_test".into(),
            program: "#program base.\nnum(10).\n#program step(t).\nadd(N) :- num(N), t == 1.\nupdate_val(N) :- num(N), t == 2.".into(),
            setup: vec![],
            maps: vec![],
            runs: 0,
            checks: vec![],
            expect_error: false,
            assert_eq: vec![
                EqAssertSpec {
                    query: "SELECT val::text FROM accum LIMIT 1".into(),
                    expected: "20".into(),
                },
            ],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![
                "DELETE FROM accum".into(),
            ],
            seed: Some(1),
            steps: Some(2),
            step_blocks: vec![
                ScenarioStepSpec {
                    setup: vec![],
                    maps: vec![
                        ScenarioMapSpec {
                            atom_name: "add".into(),
                            sql: "INSERT INTO accum (val) VALUES ({1})".into(),
                            order_by: None,
                        },
                    ],
                    checks: vec![],
                    assert_eq: vec![
                        EqAssertSpec {
                            query: "SELECT val::text FROM accum LIMIT 1".into(),
                            expected: "10".into(),
                        },
                    ],
                    assert_snapshot: vec![],
                },
                ScenarioStepSpec {
                    setup: vec![],
                    maps: vec![
                        ScenarioMapSpec {
                            atom_name: "update_val".into(),
                            sql: "UPDATE accum SET val = val + {1}".into(),
                            order_by: None,
                        },
                    ],
                    checks: vec![],
                    assert_eq: vec![
                        EqAssertSpec {
                            query: "SELECT val::text FROM accum LIMIT 1".into(),
                            expected: "20".into(),
                        },
                    ],
                    assert_snapshot: vec![],
                },
            ],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        assert!(summary.results[0].passed, "multi-step data accumulation test should pass: {}", summary.results[0].message);
    }

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_multi_step_setup_checks_snapshot() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE ms_products (id serial PRIMARY KEY, name text NOT NULL);
                CREATE TABLE ms_orders (id serial PRIMARY KEY, product_id int NOT NULL REFERENCES ms_products(id), qty int NOT NULL);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "step_features".into(),
            program: "#program base.\nproduct(widget).\n#program step(t).\ncreate(P) :- product(P), t == 1.\norder(P, 5) :- product(P), t == 2.".into(),
            setup: vec![],
            maps: vec![],
            runs: 0,
            checks: vec![],
            expect_error: false,
            assert_eq: vec![
                EqAssertSpec {
                    query: "SELECT COUNT(*)::text FROM ms_orders".into(),
                    expected: "1".into(),
                },
            ],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![
                "DELETE FROM ms_orders".into(),
                "DELETE FROM ms_products".into(),
            ],
            seed: Some(1),
            steps: Some(2),
            step_blocks: vec![
                ScenarioStepSpec {
                    setup: vec![
                        "INSERT INTO ms_products (name) VALUES ('seed_product')".into(),
                    ],
                    maps: vec![
                        ScenarioMapSpec {
                            atom_name: "create".into(),
                            sql: "INSERT INTO ms_products (name) VALUES ('{1}')".into(),
                            order_by: None,
                        },
                    ],
                    checks: vec![
                        InvariantSpec {
                            name: "products_exist".into(),
                            asserts: vec!["SELECT COUNT(*) > 0 FROM ms_products".into()],
                        },
                    ],
                    assert_eq: vec![
                        EqAssertSpec {
                            query: "SELECT COUNT(*)::text FROM ms_products".into(),
                            expected: "2".into(),
                        },
                    ],
                    assert_snapshot: vec![],
                },
                ScenarioStepSpec {
                    setup: vec![],
                    maps: vec![
                        ScenarioMapSpec {
                            atom_name: "order".into(),
                            sql: "INSERT INTO ms_orders (product_id, qty) VALUES ((SELECT id FROM ms_products WHERE name = '{1}'), {2})".into(),
                            order_by: None,
                        },
                    ],
                    checks: vec![],
                    assert_eq: vec![],
                    assert_snapshot: vec![
                        SnapshotAssertSpec {
                            query: "SELECT p.name, o.qty::text FROM ms_orders o JOIN ms_products p ON p.id = o.product_id ORDER BY p.name".into(),
                            rows: vec![vec!["widget".into(), "5".into()]],
                        },
                    ],
                },
            ],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        assert!(summary.results[0].passed, "step setup/checks/snapshot test should pass: {}", summary.results[0].message);
    }

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_multi_step_base_maps() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE bm_items (id serial PRIMARY KEY, name text NOT NULL);
                CREATE TABLE bm_tags (id serial PRIMARY KEY, item_name text NOT NULL, tag text NOT NULL);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "base_maps".into(),
            program: "#program base.\nitem(apple).\n#program step(t).\ntag(I, fresh) :- item(I), t == 1.".into(),
            setup: vec![],
            maps: vec![
                ScenarioMapSpec {
                    atom_name: "item".into(),
                    sql: "INSERT INTO bm_items (name) VALUES ('{1}')".into(),
                    order_by: None,
                },
            ],
            runs: 0,
            checks: vec![],
            expect_error: false,
            assert_eq: vec![
                EqAssertSpec {
                    query: "SELECT COUNT(*)::text FROM bm_tags".into(),
                    expected: "1".into(),
                },
            ],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![
                "DELETE FROM bm_tags".into(),
                "DELETE FROM bm_items".into(),
            ],
            seed: Some(1),
            steps: Some(1),
            step_blocks: vec![
                ScenarioStepSpec {
                    setup: vec![],
                    maps: vec![
                        ScenarioMapSpec {
                            atom_name: "tag".into(),
                            sql: "INSERT INTO bm_tags (item_name, tag) VALUES ('{1}', '{2}')".into(),
                            order_by: None,
                        },
                    ],
                    checks: vec![],
                    assert_eq: vec![],
                    assert_snapshot: vec![],
                },
            ],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        assert!(summary.results[0].passed, "base maps + step test should pass: {}", summary.results[0].message);
    }

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_multi_step_assertion_failure() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE af_data (id serial PRIMARY KEY, val int NOT NULL);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "step_fail".into(),
            program: "#program base.\nnum(1).\n#program step(t).\nadd(N) :- num(N), t == 1.".into(),
            setup: vec![],
            maps: vec![],
            runs: 0,
            checks: vec![],
            expect_error: false,
            assert_eq: vec![],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![
                "DELETE FROM af_data".into(),
            ],
            seed: Some(1),
            steps: Some(1),
            step_blocks: vec![
                ScenarioStepSpec {
                    setup: vec![],
                    maps: vec![
                        ScenarioMapSpec {
                            atom_name: "add".into(),
                            sql: "INSERT INTO af_data (val) VALUES ({1})".into(),
                            order_by: None,
                        },
                    ],
                    checks: vec![],
                    assert_eq: vec![
                        EqAssertSpec {
                            query: "SELECT COUNT(*)::text FROM af_data".into(),
                            expected: "999".into(), // deliberately wrong
                        },
                    ],
                    assert_snapshot: vec![],
                },
            ],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        assert!(!summary.results[0].passed, "step assertion failure should fail the scenario");
        assert!(summary.results[0].message.contains("step 1"), "failure message should mention step: {}", summary.results[0].message);
        assert!(summary.results[0].message.contains("expected '999'"), "failure message should mention expected value: {}", summary.results[0].message);
    }

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_multi_step_check_failure() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE cf_data (id serial PRIMARY KEY, val int NOT NULL);
            ").unwrap();
        }

        let scenario = ScenarioSpec {
            name: "step_check_fail".into(),
            program: "#program base.\nnum(1).\n#program step(t).\nadd(N) :- num(N), t == 1.".into(),
            setup: vec![],
            maps: vec![],
            runs: 0,
            checks: vec![],
            expect_error: false,
            assert_eq: vec![],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![
                "DELETE FROM cf_data".into(),
            ],
            seed: Some(1),
            steps: Some(1),
            step_blocks: vec![
                ScenarioStepSpec {
                    setup: vec![],
                    maps: vec![
                        ScenarioMapSpec {
                            atom_name: "add".into(),
                            sql: "INSERT INTO cf_data (val) VALUES ({1})".into(),
                            order_by: None,
                        },
                    ],
                    checks: vec![
                        InvariantSpec {
                            name: "impossible_check".into(),
                            asserts: vec!["SELECT false".into()], // always fails
                        },
                    ],
                    assert_eq: vec![],
                    assert_snapshot: vec![],
                },
            ],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        assert!(!summary.results[0].passed, "step check failure should fail the scenario");
        assert!(summary.results[0].message.contains("step 1"), "failure message should mention step: {}", summary.results[0].message);
        assert!(summary.results[0].message.contains("impossible_check"), "failure message should mention check name: {}", summary.results[0].message);
    }

    #[test]
    #[cfg(feature = "scenario")]
    fn scenario_multi_step_expect_error() {
        let (_c, dsn) = start_pg();

        {
            let mut setup_client = Client::connect(&dsn, NoTls).unwrap();
            setup_client.batch_execute("
                CREATE TABLE ee_data (id serial PRIMARY KEY, val int NOT NULL UNIQUE);
            ").unwrap();
        }

        // Step 1 inserts val=1, step 2 tries to insert duplicate val=1 → expect_error
        let scenario = ScenarioSpec {
            name: "multi_expect_error".into(),
            program: "#program base.\nnum(1).\n#program step(t).\nadd(N) :- num(N), t == 1.\ndup(N) :- num(N), t == 2.".into(),
            setup: vec![],
            maps: vec![],
            runs: 0,
            checks: vec![],
            expect_error: true,
            assert_eq: vec![],
            assert_snapshot: vec![],
            params: vec![],
            teardown: vec![
                "DELETE FROM ee_data".into(),
            ],
            seed: Some(1),
            steps: Some(2),
            step_blocks: vec![
                ScenarioStepSpec {
                    setup: vec![],
                    maps: vec![
                        ScenarioMapSpec {
                            atom_name: "add".into(),
                            sql: "INSERT INTO ee_data (val) VALUES ({1})".into(),
                            order_by: None,
                        },
                    ],
                    checks: vec![],
                    assert_eq: vec![],
                    assert_snapshot: vec![],
                },
                ScenarioStepSpec {
                    setup: vec![],
                    maps: vec![
                        ScenarioMapSpec {
                            atom_name: "dup".into(),
                            sql: "INSERT INTO ee_data (val) VALUES ({1})".into(), // duplicate unique constraint
                            order_by: None,
                        },
                    ],
                    checks: vec![],
                    assert_eq: vec![],
                    assert_snapshot: vec![],
                },
            ],
        };

        let cfg = Config {
            scenarios: vec![scenario],
            ..Default::default()
        };

        let summary = PostgresTestBackend.run(&cfg, &dsn, None).unwrap();
        assert_eq!(summary.results.len(), 1);
        assert!(summary.results[0].passed, "multi-step expect_error should pass when SQL errors: {}", summary.results[0].message);
        assert!(summary.results[0].message.contains("expected error"), "message should mention expected error: {}", summary.results[0].message);
    }
}
