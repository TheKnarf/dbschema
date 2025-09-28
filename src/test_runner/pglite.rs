use std::collections::HashSet;
use std::str;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use bytes::BytesMut;
use fallible_iterator::FallibleIterator;
use log::info;
use pglite_oxide::interactive::{self, PokeInput};
use postgres_protocol::message::backend::{DataRowBody, ErrorResponseBody, Message};

use super::{is_verbose, TestBackend, TestResult, TestSummary};
use crate::ir::Config;

const RETRY_WAIT: Duration = Duration::from_millis(50);
const RETRY_TIMEOUT: Duration = Duration::from_secs(5);

pub struct PgliteTestBackend;

impl TestBackend for PgliteTestBackend {
    fn run(&self, cfg: &Config, dsn: &str, only: Option<&HashSet<String>>) -> Result<TestSummary> {
        if !dsn.trim().is_empty() {
            return Err(anyhow!(
                "pglite backend does not support custom DSN overrides"
            ));
        }

        let _mount = pglite_oxide::prepare_default_mount()?;
        interactive::with_default_runtime(|rt| rt.ensure_handshake())?;

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
            let mut began = false;

            if let Err(err) = exec_statement("BEGIN") {
                ok = false;
                failed_msg = format!("failed to begin transaction: {err}");
            } else {
                began = true;
            }

            if ok {
                for s in &t.setup {
                    if is_verbose() {
                        info!("-- setup: {}", s);
                    }
                    if let Err(err) = exec_statement(s) {
                        ok = false;
                        failed_msg = format!("setup failed: {err}");
                        break;
                    }
                }
            }

            if ok {
                for a in &t.asserts {
                    if is_verbose() {
                        info!("-- assert: {}", a);
                    }
                    match exec_query_bool(a) {
                        Ok(true) => {}
                        Ok(false) => {
                            ok = false;
                            failed_msg = "assert returned false".into();
                            break;
                        }
                        Err(err) => {
                            ok = false;
                            failed_msg = format!("assert error: {err}");
                            break;
                        }
                    }
                }
            }

            if ok {
                for a in &t.assert_fail {
                    if is_verbose() {
                        info!("-- assert-fail: {}", a);
                    }
                    match exec_expect_error(a) {
                        Ok(()) => {}
                        Err(err) => {
                            ok = false;
                            failed_msg = format!("assert-fail succeeded unexpectedly: {err}");
                            break;
                        }
                    }
                }
            }

            if began {
                let _ = exec_statement("ROLLBACK");
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
}

struct ParsedResponse {
    error: Option<String>,
    data_row: Option<Vec<Option<Vec<u8>>>>,
}

fn exec_statement(sql: &str) -> Result<()> {
    let response = exec_with_retry(sql)?;
    if let Some(err) = response.error {
        Err(anyhow!(err))
    } else {
        Ok(())
    }
}

fn exec_query_bool(sql: &str) -> Result<bool> {
    let response = exec_with_retry(sql)?;
    if let Some(err) = response.error {
        return Err(anyhow!(err));
    }
    let row = response.data_row.unwrap_or_default();
    let value = row.into_iter().next().unwrap_or(None);
    decode_bool(value)
}

fn exec_expect_error(sql: &str) -> Result<()> {
    let response = exec_with_retry(sql)?;
    if let Some(_) = response.error {
        Ok(())
    } else {
        Err(anyhow!("statement succeeded unexpectedly"))
    }
}

fn exec_with_retry(sql: &str) -> Result<ParsedResponse> {
    let deadline = Instant::now() + RETRY_TIMEOUT;
    loop {
        match exec_sql(sql) {
            Ok(resp) => return Ok(resp),
            Err(err) => {
                if Instant::now() >= deadline {
                    return Err(err);
                }
                thread::sleep(RETRY_WAIT);
            }
        }
    }
}

fn exec_sql(sql: &str) -> Result<ParsedResponse> {
    let bytes = interactive::with_default_runtime(|rt| rt.exec_interactive(PokeInput::Str(sql)))
        .with_context(|| format!("executing '{sql}'"))?;
    parse_response(&bytes)
}

fn parse_response(buf: &[u8]) -> Result<ParsedResponse> {
    let mut bytes = BytesMut::from(buf);
    let mut error = None;
    let mut data_row = None;

    while !bytes.is_empty() {
        match Message::parse(&mut bytes).map_err(|e| anyhow!(e))? {
            Some(Message::ErrorResponse(body)) => {
                error = Some(parse_error(body)?);
            }
            Some(Message::DataRow(body)) => {
                if data_row.is_none() {
                    data_row = Some(parse_data_row(body)?);
                }
            }
            Some(Message::ReadyForQuery(_)) => break,
            Some(_) => {}
            None => break,
        }
    }

    Ok(ParsedResponse { error, data_row })
}

fn parse_error(body: ErrorResponseBody) -> Result<String> {
    let mut fields = body.fields();
    let mut message = String::from("unknown error");
    while let Some(field) = fields.next()? {
        if field.type_() == b'M' {
            message = str::from_utf8(field.value_bytes())
                .unwrap_or("unknown error")
                .to_string();
        }
    }
    Ok(message)
}

fn parse_data_row(body: DataRowBody) -> Result<Vec<Option<Vec<u8>>>> {
    let mut values = Vec::new();
    let buffer = body.buffer();
    let mut ranges = body.ranges();
    while let Some(range) = ranges.next()? {
        match range {
            Some(range) => values.push(Some(buffer[range.start..range.end].to_vec())),
            None => values.push(None),
        }
    }
    Ok(values)
}

fn decode_bool(field: Option<Vec<u8>>) -> Result<bool> {
    match field {
        None => Ok(false),
        Some(bytes) => {
            if bytes.is_empty() {
                return Ok(false);
            }
            if bytes == b"t" || bytes.eq_ignore_ascii_case(b"true") {
                return Ok(true);
            }
            if bytes == b"f" || bytes.eq_ignore_ascii_case(b"false") {
                return Ok(false);
            }
            if let Ok(text) = str::from_utf8(&bytes) {
                if let Ok(i) = text.trim().parse::<i64>() {
                    return Ok(i != 0);
                }
            }
            Err(anyhow!("unsupported assert result type"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PgliteTestBackend;
    use crate::ir::{Config, TestSpec};
    use crate::test_runner::TestBackend;

    #[test]
    fn runs_simple_test() {
        let backend = PgliteTestBackend;
        let config = Config {
            tests: vec![TestSpec {
                name: "basic-select".into(),
                setup: vec![
                    "CREATE TABLE numbers(value INTEGER);".into(),
                    "INSERT INTO numbers VALUES (1);".into(),
                ],
                asserts: vec![
                    "SELECT COUNT(*) = 1 FROM numbers".into(),
                    "SELECT value = 1 FROM numbers LIMIT 1".into(),
                ],
                assert_fail: vec!["INSERT INTO numbers(value) VALUES (1/0);".into()],
                teardown: Vec::new(),
            }],
            ..Config::default()
        };

        let summary = backend
            .run(&config, "", None)
            .expect("pglite backend should run");
        assert_eq!(summary.total, 1);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 0);
    }
}
