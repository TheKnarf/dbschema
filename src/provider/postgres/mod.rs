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
}
