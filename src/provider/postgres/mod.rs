pub mod backend;
pub mod test_backend;

use crate::provider::Provider;

/// PostgreSQL database provider.
pub struct PostgresProvider;

impl PostgresProvider {
    /// Create a new PostgreSQL provider instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for PostgresProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for PostgresProvider {
    fn name(&self) -> &str {
        "postgres"
    }

    fn register_resources(&self) {
        // TODO: Register PostgreSQL-specific resource types:
        // - table
        // - function
        // - trigger
        // - view
        // - index
        // - constraint
        // - etc.
    }

    fn register_backends(&self, registry: &mut crate::backends::BackendRegistry) {
        // Register the main postgres backend
        registry.register(Box::new(backend::PostgresBackend));

        // Register "pg" as an alias
        registry.register_alias("pg", Box::new(backend::PostgresBackend));
    }

    fn register_test_backends(&self, registry: &mut crate::test_runner::TestBackendRegistry) {
        // Register the postgres test backend
        registry.register("postgres", Box::new(test_backend::PostgresTestBackend));

        // Register "pg" as an alias
        registry.register("pg", Box::new(test_backend::PostgresTestBackend));
    }
}
