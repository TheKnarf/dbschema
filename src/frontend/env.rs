use anyhow::{bail, Result};
use hcl::Value;
use std::collections::HashMap;

use super::ast::VarValidation;

/// Variables available during expression evaluation.
///
/// The evaluation engine exposes several namespaces:
/// - `var.<name>` for values in [`Self::vars`]
/// - `local.<name>` or `locals.<name>` for values in [`Self::locals`]
/// - `module.<name>.<output>` for outputs produced by modules
/// - `data.<type>.<name>` for values loaded from data sources
/// - `each.key`/`each.value` inside `for_each` blocks
/// - `count.index` inside blocks using the `count` attribute
///
/// # Example
/// ```
/// use dbschema::frontend::env::EnvVars;
/// use hcl::Value;
/// use std::collections::HashMap;
///
/// let env = EnvVars {
///     vars: HashMap::from([( "name".into(), Value::from("world"))]),
///     locals: HashMap::from([( "name".into(), Value::from("bob"))]),
///     modules: HashMap::new(),
///     data: HashMap::new(),
///     each: None,
///     count: None,
/// };
/// // `local.name` resolves to "bob" while `var.name` resolves to "world".
/// ```
#[derive(Default, Clone, Debug)]
pub struct EnvVars {
    /// Variables passed from the outside world, resolved as `var.*`.
    pub vars: HashMap<String, Value>,
    /// Locally defined values, resolved as `local.*` or `locals.*`.
    pub locals: HashMap<String, Value>,
    /// Outputs from loaded modules, referenced as `module.<name>.<output>`.
    pub modules: HashMap<String, HashMap<String, Value>>,
    /// Values loaded via `data` blocks, referenced as `data.<type>.<name>`.
    pub data: HashMap<String, HashMap<String, Value>>,
    /// Key/value for the current iteration of a `for_each` block, enabling `each.key` and `each.value`.
    pub each: Option<(Value, Value)>, // (key, value)
    /// Index for `count`-based iterations, enabling `count.index`.
    pub count: Option<usize>,
}

#[derive(Clone, Debug)]
pub enum VarType {
    String,
    Number,
    Bool,
    List(Box<VarType>),
    Map(Box<VarType>),
}

impl std::fmt::Display for VarType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VarType::String => write!(f, "string"),
            VarType::Number => write!(f, "number"),
            VarType::Bool => write!(f, "bool"),
            VarType::List(inner) => write!(f, "list({inner})"),
            VarType::Map(inner) => write!(f, "map({inner})"),
        }
    }
}

impl std::str::FromStr for VarType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();
        if let Some(inner) = s.strip_prefix("list(").and_then(|v| v.strip_suffix(')')) {
            return Ok(VarType::List(Box::new(inner.parse()?)));
        }
        if let Some(inner) = s.strip_prefix("array(").and_then(|v| v.strip_suffix(')')) {
            return Ok(VarType::List(Box::new(inner.parse()?)));
        }
        if let Some(inner) = s.strip_prefix("map(").and_then(|v| v.strip_suffix(')')) {
            return Ok(VarType::Map(Box::new(inner.parse()?)));
        }
        if let Some(inner) = s.strip_prefix("object(").and_then(|v| v.strip_suffix(')')) {
            return Ok(VarType::Map(Box::new(inner.parse()?)));
        }
        match s {
            "string" => Ok(VarType::String),
            "number" => Ok(VarType::Number),
            "bool" | "boolean" => Ok(VarType::Bool),
            _ => bail!("unknown type '{s}'"),
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct VarSpec {
    pub default: Option<Value>,
    pub r#type: Option<VarType>,
    pub validation: Option<VarValidation>,
}
