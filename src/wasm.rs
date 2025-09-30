use wasm_bindgen::prelude::*;
use std::path::Path;
use std::collections::HashMap;

use crate::{load_config, validate, generate_with_backend, apply_filters, Loader};
use crate::frontend::env::EnvVars;
use crate::config::ResourceKind;

/// JavaScript callback function type for loading files
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "(path: string) => string")]
    pub type LoaderCallback;

    #[wasm_bindgen(method, structural, js_name = call)]
    fn call(this: &LoaderCallback, path: &str) -> String;
}

struct JsLoader {
    callback: LoaderCallback,
}

impl Loader for JsLoader {
    fn load(&self, path: &Path) -> anyhow::Result<String> {
        // Convert path to string using display() which always works
        let path_str = path.display().to_string();

        // Debug: log the path we're trying to load
        eprintln!("[RUST DEBUG] load() called with path: {:?}, display: {}", path, path_str);

        if path_str.is_empty() {
            anyhow::bail!("Attempted to load file with empty path");
        }

        let content = self.callback.call(&path_str);
        Ok(content)
    }
}

/// Options for validation
#[wasm_bindgen]
#[derive(Default)]
pub struct ValidateOptions {
    strict: bool,
    include_resources: Vec<String>,
    exclude_resources: Vec<String>,
}

#[wasm_bindgen]
impl ValidateOptions {
    #[wasm_bindgen(constructor)]
    pub fn new() -> ValidateOptions {
        ValidateOptions::default()
    }

    #[wasm_bindgen(setter)]
    pub fn set_strict(&mut self, strict: bool) {
        self.strict = strict;
    }

    #[wasm_bindgen(setter)]
    pub fn set_include_resources(&mut self, resources: Vec<String>) {
        self.include_resources = resources;
    }

    #[wasm_bindgen(setter)]
    pub fn set_exclude_resources(&mut self, resources: Vec<String>) {
        self.exclude_resources = resources;
    }
}

/// Result of validation
#[wasm_bindgen]
pub struct ValidateResult {
    success: bool,
    error: Option<String>,
    summary: String,
}

#[wasm_bindgen]
impl ValidateResult {
    #[wasm_bindgen(getter)]
    pub fn success(&self) -> bool {
        self.success
    }

    #[wasm_bindgen(getter)]
    pub fn error(&self) -> Option<String> {
        self.error.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn summary(&self) -> String {
        self.summary.clone()
    }
}

/// Options for generation
#[wasm_bindgen]
#[derive(Default)]
pub struct GenerateOptions {
    backend: String,
    strict: bool,
    include_resources: Vec<String>,
    exclude_resources: Vec<String>,
}

#[wasm_bindgen]
impl GenerateOptions {
    #[wasm_bindgen(constructor)]
    pub fn new(backend: String) -> GenerateOptions {
        GenerateOptions {
            backend,
            ..Default::default()
        }
    }

    #[wasm_bindgen(setter)]
    pub fn set_strict(&mut self, strict: bool) {
        self.strict = strict;
    }

    #[wasm_bindgen(setter)]
    pub fn set_include_resources(&mut self, resources: Vec<String>) {
        self.include_resources = resources;
    }

    #[wasm_bindgen(setter)]
    pub fn set_exclude_resources(&mut self, resources: Vec<String>) {
        self.exclude_resources = resources;
    }
}

fn parse_resource_kind(s: &str) -> Option<ResourceKind> {
    match s.to_lowercase().as_str() {
        "schemas" => Some(ResourceKind::Schemas),
        "enums" => Some(ResourceKind::Enums),
        "domains" => Some(ResourceKind::Domains),
        "types" => Some(ResourceKind::Types),
        "tables" => Some(ResourceKind::Tables),
        "views" => Some(ResourceKind::Views),
        "materialized" => Some(ResourceKind::Materialized),
        "functions" => Some(ResourceKind::Functions),
        "procedures" => Some(ResourceKind::Procedures),
        "aggregates" => Some(ResourceKind::Aggregates),
        "operators" => Some(ResourceKind::Operators),
        "triggers" => Some(ResourceKind::Triggers),
        "rules" => Some(ResourceKind::Rules),
        "eventtriggers" | "event_triggers" => Some(ResourceKind::EventTriggers),
        "extensions" => Some(ResourceKind::Extensions),
        "collations" => Some(ResourceKind::Collations),
        "sequences" => Some(ResourceKind::Sequences),
        "policies" => Some(ResourceKind::Policies),
        "roles" => Some(ResourceKind::Roles),
        "tablespaces" => Some(ResourceKind::Tablespaces),
        "grants" => Some(ResourceKind::Grants),
        "tests" => Some(ResourceKind::Tests),
        "publications" => Some(ResourceKind::Publications),
        "subscriptions" => Some(ResourceKind::Subscriptions),
        _ => None,
    }
}

fn build_filter_sets(
    include: &[String],
    exclude: &[String],
) -> (std::collections::HashSet<ResourceKind>, std::collections::HashSet<ResourceKind>) {
    let include_set: std::collections::HashSet<ResourceKind> = if include.is_empty() {
        // Default: include all resource types
        vec![
            ResourceKind::Schemas,
            ResourceKind::Enums,
            ResourceKind::Domains,
            ResourceKind::Types,
            ResourceKind::Tables,
            ResourceKind::Views,
            ResourceKind::Materialized,
            ResourceKind::Functions,
            ResourceKind::Procedures,
            ResourceKind::Aggregates,
            ResourceKind::Operators,
            ResourceKind::Triggers,
            ResourceKind::Rules,
            ResourceKind::EventTriggers,
            ResourceKind::Extensions,
            ResourceKind::Collations,
            ResourceKind::Sequences,
            ResourceKind::Policies,
            ResourceKind::Roles,
            ResourceKind::Tablespaces,
            ResourceKind::Grants,
            ResourceKind::Tests,
            ResourceKind::Publications,
            ResourceKind::Subscriptions,
        ]
        .into_iter()
        .collect()
    } else {
        include
            .iter()
            .filter_map(|s| parse_resource_kind(s))
            .collect()
    };

    let exclude_set: std::collections::HashSet<ResourceKind> = exclude
        .iter()
        .filter_map(|s| parse_resource_kind(s))
        .collect();

    (include_set, exclude_set)
}

/// Validate HCL configuration
///
/// # Arguments
/// * `root_path` - Path to the root HCL file (e.g., "main.hcl")
/// * `loader` - JavaScript callback function that takes a path string and returns file contents
/// * `options` - Optional validation options
///
/// # Example
/// ```javascript
/// const result = validate("main.hcl", (path) => fs.readFileSync(path, 'utf-8'), options);
/// if (result.success) {
///   console.log(result.summary);
/// } else {
///   console.error(result.error);
/// }
/// ```
#[wasm_bindgen]
pub fn validate_hcl(
    root_path: &str,
    loader: LoaderCallback,
    options: Option<ValidateOptions>,
) -> ValidateResult {
    eprintln!("[RUST DEBUG] validate_hcl called with root_path: {:?}", root_path);

    let opts = options.unwrap_or_default();
    let js_loader = JsLoader { callback: loader };

    let env = EnvVars {
        vars: HashMap::new(),
        locals: HashMap::new(),
        modules: HashMap::new(),
        each: None,
        count: None,
    };

    eprintln!("[RUST DEBUG] About to call load_config with Path::new({:?})", root_path);
    let config = match load_config(Path::new(root_path), &js_loader, env) {
        Ok(cfg) => cfg,
        Err(e) => {
            return ValidateResult {
                success: false,
                error: Some(format!("Failed to load config: {}", e)),
                summary: String::new(),
            };
        }
    };

    let (include_set, exclude_set) = build_filter_sets(&opts.include_resources, &opts.exclude_resources);
    let filtered = apply_filters(&config, &include_set, &exclude_set);

    match validate(&filtered, opts.strict) {
        Ok(()) => {
            let summary = format!(
                "Valid: {} schema(s), {} enum(s), {} table(s), {} view(s), {} materialized view(s), {} function(s), {} procedure(s), {} trigger(s)",
                filtered.schemas.len(),
                filtered.enums.len(),
                filtered.tables.len(),
                filtered.views.len(),
                filtered.materialized.len(),
                filtered.functions.len(),
                filtered.procedures.len(),
                filtered.triggers.len()
            );
            ValidateResult {
                success: true,
                error: None,
                summary,
            }
        }
        Err(e) => ValidateResult {
            success: false,
            error: Some(format!("Validation failed: {}", e)),
            summary: String::new(),
        },
    }
}

/// Generate SQL or other backend output from HCL configuration
///
/// # Arguments
/// * `root_path` - Path to the root HCL file (e.g., "main.hcl")
/// * `loader` - JavaScript callback function that takes a path string and returns file contents
/// * `options` - Generation options including backend type
///
/// # Example
/// ```javascript
/// const sql = generate("main.hcl", (path) => fs.readFileSync(path, 'utf-8'), options);
/// console.log(sql);
/// ```
#[wasm_bindgen]
pub fn generate(
    root_path: &str,
    loader: LoaderCallback,
    options: GenerateOptions,
) -> Result<String, JsValue> {
    let js_loader = JsLoader { callback: loader };

    let env = EnvVars {
        vars: HashMap::new(),
        locals: HashMap::new(),
        modules: HashMap::new(),
        each: None,
        count: None,
    };

    let config = load_config(Path::new(root_path), &js_loader, env)
        .map_err(|e| JsValue::from_str(&format!("Failed to load config: {}", e)))?;

    let (include_set, exclude_set) = build_filter_sets(&options.include_resources, &options.exclude_resources);
    let filtered = apply_filters(&config, &include_set, &exclude_set);

    validate(&filtered, options.strict)
        .map_err(|e| JsValue::from_str(&format!("Validation failed: {}", e)))?;

    let output = generate_with_backend(&options.backend, &filtered, options.strict)
        .map_err(|e| JsValue::from_str(&format!("Generation failed: {}", e)))?;

    Ok(output)
}

/// Format HCL content
///
/// # Arguments
/// * `content` - HCL content to format
///
/// # Returns
/// Formatted HCL string or error
#[wasm_bindgen]
pub fn format_hcl(content: &str) -> Result<String, JsValue> {
    let body: hcl::Body = hcl::from_str(content)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse HCL: {}", e)))?;

    let formatted = hcl::format::to_string(&body)
        .map_err(|e| JsValue::from_str(&format!("Failed to format HCL: {}", e)))?;

    Ok(formatted)
}

/// Get version information
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}