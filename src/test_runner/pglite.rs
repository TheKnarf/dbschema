use anyhow::{anyhow, Result};
use std::collections::HashSet;

use super::{TestBackend, TestSummary};
use crate::ir::Config;

/// In-memory Postgres backend powered by the PGlite WASM build.
///
/// At the moment this is only a stub that documents the intended
/// integration. The full WASM runtime and SQL execution logic still
/// needs to be implemented.
pub struct PGliteTestBackend;

impl TestBackend for PGliteTestBackend {
    fn run(
        &self,
        _cfg: &Config,
        _dsn: &str,
        _only: Option<&HashSet<String>>,
    ) -> Result<TestSummary> {
        Err(anyhow!("PGlite backend not implemented"))
    }
}
