use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::ir::Config;

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

    /// Returns `true` if this backend knows how to provision temporary databases.
    fn supports_temporary_database(&self) -> bool {
        false
    }

    /// Create (or recreate) a temporary database and return the DSN pointing to it.
    ///
    /// The default implementation returns an error indicating that temporary databases are not supported.
    fn setup_temporary_database(&self, _dsn: &str, _database_name: &str, _verbose: bool) -> Result<String> {
        Err(anyhow!(
            "temporary databases are not supported by this test backend"
        ))
    }

    /// Drop the temporary database that was previously created via [`setup_temporary_database`].
    ///
    /// Implementations should tolerate best-effort cleanup.
    fn cleanup_temporary_database(&self, _dsn: &str, _database_name: &str, _verbose: bool) -> Result<()> {
        Ok(())
    }
}

/// Registry for managing test backends provided by providers.
pub struct TestBackendRegistry {
    backends: HashMap<String, Box<dyn TestBackend>>,
}

impl TestBackendRegistry {
    /// Create a new empty test backend registry.
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
        }
    }

    /// Register a test backend with the registry.
    pub fn register(&mut self, name: &str, backend: Box<dyn TestBackend>) {
        self.backends.insert(name.to_lowercase(), backend);
    }

    /// Get a test backend by name (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&dyn TestBackend> {
        self.backends.get(&name.to_lowercase()).map(|b| &**b)
    }

    /// List all registered test backend names.
    pub fn list_backends(&self) -> Vec<&str> {
        self.backends.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for TestBackendRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a test backend registry with all test backends from registered providers.
pub fn get_default_test_backend_registry() -> TestBackendRegistry {
    let mut registry = TestBackendRegistry::new();

    // Register provider test backends
    let provider_registry = crate::provider::get_default_provider_registry();
    for provider_name in provider_registry.list_providers() {
        if let Some(provider) = provider_registry.get(provider_name) {
            provider.register_test_backends(&mut registry);
        }
    }

    registry
}

static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn set_verbose(v: bool) {
    VERBOSE.store(v, Ordering::Relaxed);
}

pub fn is_verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}
