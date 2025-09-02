use anyhow::Result;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;

/// Global settings for dbschema
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    /// Default input file if not specified in targets
    pub input: Option<String>,
    /// Default variable files
    #[serde(default)]
    pub var_files: Vec<String>,
    /// Environment variables to set
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Default backend for `dbschema test`
    #[serde(default)]
    pub test_backend: Option<String>,
    /// Default DSN for `dbschema test`
    #[serde(default)]
    pub test_dsn: Option<String>,
}

/// Configuration for a single target output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    /// Name of the target (for identification)
    pub name: String,

    /// Backend to use for this target
    pub backend: String,

    /// Input file for this target
    pub input: Option<String>,

    /// Output file path (if not specified, prints to stdout)
    pub output: Option<String>,

    /// Resource types to include (if empty, includes all)
    #[serde(default)]
    pub include: Vec<String>,

    /// Resource types to exclude
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Variables for this target
    #[serde(default)]
    pub vars: HashMap<String, toml::Value>,

    /// Variable files for this target
    #[serde(default)]
    pub var_files: Vec<String>,

    /// Additional backend-specific options
    #[serde(flatten)]
    pub options: std::collections::HashMap<String, toml::Value>,
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Global settings
    #[serde(default)]
    pub settings: Settings,
    /// List of targets to generate
    pub targets: Vec<TargetConfig>,
}

/// Resource types that can be filtered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum)]
pub enum ResourceKind {
    Schemas,
    Enums,
    Domains,
    Types,
    Tables,
    Views,
    Materialized,
    Functions,
    Triggers,
    Extensions,
    Sequences,
    Policies,
    Roles,
    Grants,
    Tests,
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ResourceKind::Schemas => "schemas",
            ResourceKind::Enums => "enums",
            ResourceKind::Domains => "domains",
            ResourceKind::Types => "types",
            ResourceKind::Tables => "tables",
            ResourceKind::Views => "views",
            ResourceKind::Materialized => "materialized",
            ResourceKind::Functions => "functions",
            ResourceKind::Triggers => "triggers",
            ResourceKind::Extensions => "extensions",
            ResourceKind::Sequences => "sequences",
            ResourceKind::Policies => "policies",
            ResourceKind::Roles => "roles",
            ResourceKind::Grants => "grants",
            ResourceKind::Tests => "tests",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for ResourceKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "schemas" => Ok(ResourceKind::Schemas),
            "enums" => Ok(ResourceKind::Enums),
            "domains" => Ok(ResourceKind::Domains),
            "types" => Ok(ResourceKind::Types),
            "tables" => Ok(ResourceKind::Tables),
            "views" => Ok(ResourceKind::Views),
            "materialized" => Ok(ResourceKind::Materialized),
            "functions" => Ok(ResourceKind::Functions),
            "triggers" => Ok(ResourceKind::Triggers),
            "extensions" => Ok(ResourceKind::Extensions),
            "sequences" => Ok(ResourceKind::Sequences),
            "policies" => Ok(ResourceKind::Policies),
            "roles" => Ok(ResourceKind::Roles),
            "grants" => Ok(ResourceKind::Grants),
            "tests" => Ok(ResourceKind::Tests),
            _ => Err(format!("invalid resource kind: {}", s)),
        }
    }
}

impl TargetConfig {
    /// Get the set of resource kinds to include
    pub fn get_include_set(&self) -> HashSet<ResourceKind> {
        if self.include.is_empty() {
            // Include all by default
            vec![
                ResourceKind::Schemas,
                ResourceKind::Enums,
                ResourceKind::Domains,
                ResourceKind::Types,
                ResourceKind::Tables,
                ResourceKind::Views,
                ResourceKind::Materialized,
                ResourceKind::Functions,
                ResourceKind::Triggers,
                ResourceKind::Extensions,
                ResourceKind::Sequences,
                ResourceKind::Policies,
                ResourceKind::Roles,
                ResourceKind::Grants,
                ResourceKind::Tests,
            ]
            .into_iter()
            .collect()
        } else {
            self.include
                .iter()
                .filter_map(|s| s.parse::<ResourceKind>().ok())
                .collect()
        }
    }

    /// Get the set of resource kinds to exclude
    pub fn get_exclude_set(&self) -> HashSet<ResourceKind> {
        self.exclude
            .iter()
            .filter_map(|s| s.parse::<ResourceKind>().ok())
            .collect()
    }
}

/// Load configuration from dbschema.toml file
pub fn load_config() -> Result<Option<Config>> {
    load_config_from_path(Path::new("dbschema.toml"))
}

/// Load configuration from a specific path
pub fn load_config_from_path(path: &Path) -> Result<Option<Config>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(Some(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_kind_from_str() {
        assert_eq!("tables".parse::<ResourceKind>(), Ok(ResourceKind::Tables));
        assert_eq!("TABLES".parse::<ResourceKind>(), Ok(ResourceKind::Tables));
        assert!("invalid".parse::<ResourceKind>().is_err());
    }

    #[test]
    fn test_target_config_include_all() {
        let target = TargetConfig {
            name: "test".to_string(),
            backend: "postgres".to_string(),
            input: None,
            output: None,
            include: vec![],
            exclude: vec![],
            vars: Default::default(),
            var_files: vec![],
            options: Default::default(),
        };

        let include_set = target.get_include_set();
        assert!(include_set.contains(&ResourceKind::Tables));
        assert!(include_set.contains(&ResourceKind::Enums));
        assert_eq!(include_set.len(), 15); // All resource types
    }

    #[test]
    fn test_target_config_include_specific() {
        let target = TargetConfig {
            name: "test".to_string(),
            backend: "prisma".to_string(),
            input: None,
            output: None,
            include: vec!["tables".to_string(), "enums".to_string()],
            exclude: vec![],
            vars: Default::default(),
            var_files: vec![],
            options: Default::default(),
        };

        let include_set = target.get_include_set();
        assert!(include_set.contains(&ResourceKind::Tables));
        assert!(include_set.contains(&ResourceKind::Enums));
        assert!(!include_set.contains(&ResourceKind::Functions));
        assert_eq!(include_set.len(), 2);
    }

    #[test]
    fn test_target_config_exclude() {
        let target = TargetConfig {
            name: "test".to_string(),
            backend: "postgres".to_string(),
            input: None,
            output: None,
            include: vec![],
            exclude: vec!["functions".to_string(), "triggers".to_string()],
            vars: Default::default(),
            var_files: vec![],
            options: Default::default(),
        };

        let include_set = target.get_include_set();
        let exclude_set = target.get_exclude_set();

        assert!(include_set.contains(&ResourceKind::Tables));
        assert!(!exclude_set.contains(&ResourceKind::Tables));
        assert!(exclude_set.contains(&ResourceKind::Functions));
        assert!(exclude_set.contains(&ResourceKind::Triggers));
    }
}
