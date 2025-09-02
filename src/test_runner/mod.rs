use anyhow::Result;
use std::collections::HashSet;

use crate::ir::Config;

pub mod postgres;
#[cfg(feature = "pglite")]
pub mod pglite;

pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub message: String,
}

pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<TestResult>,
}

pub trait TestBackend {
    fn run(&self, cfg: &Config, dsn: &str, only: Option<&HashSet<String>>) -> Result<TestSummary>;
}
