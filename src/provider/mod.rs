pub mod postgres;

use std::collections::HashMap;

/// A provider defines database-specific resource types and SQL generation.
pub trait Provider: Send + Sync {
    /// Returns the name of this provider (e.g., "postgres", "sqlite", "mysql")
    fn name(&self) -> &str;

    /// Register all resource types supported by this provider.
    /// This will be expanded later to use a ResourceRegistry.
    fn register_resources(&self) {
        // Placeholder - will be implemented with resource schema system
    }

    /// Register all backends provided by this provider (0, 1, or many).
    /// Providers can register multiple backends with different names.
    fn register_backends(&self, registry: &mut crate::backends::BackendRegistry);
}

/// Registry for managing database providers.
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
}

impl ProviderRegistry {
    /// Create a new empty provider registry.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register a provider with the registry.
    pub fn register(&mut self, provider: Box<dyn Provider>) {
        let name = provider.name().to_string();
        self.providers.insert(name, provider);
    }

    /// Get a provider by name.
    pub fn get(&self, name: &str) -> Option<&dyn Provider> {
        self.providers.get(name).map(|p| &**p)
    }

    /// List all registered provider names.
    pub fn list_providers(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a provider registry with all built-in providers registered.
pub fn get_default_provider_registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(postgres::PostgresProvider::new()));
    registry
}
