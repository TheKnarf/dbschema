use anyhow::Result;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::ir::Config;

pub mod postgres;

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

static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn set_verbose(v: bool) {
    VERBOSE.store(v, Ordering::Relaxed);
}

pub fn is_verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}
