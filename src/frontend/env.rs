use hcl::Value;
use std::collections::HashMap;

use super::ast::VarValidation;

/// Variables available during expression evaluation.
///
/// The evaluation engine exposes several namespaces:
/// - `var.<name>` for values in [`Self::vars`]
/// - `local.<name>` or `locals.<name>` for values in [`Self::locals`]
/// - `module.<name>.<output>` for outputs produced by modules
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
    /// Key/value for the current iteration of a `for_each` block, enabling `each.key` and `each.value`.
    pub each: Option<(Value, Value)>, // (key, value)
    /// Index for `count`-based iterations, enabling `count.index`.
    pub count: Option<usize>,
}

#[derive(Default, Clone)]
pub struct VarSpec {
    pub default: Option<Value>,
    pub r#type: Option<String>,
    pub validation: Option<VarValidation>,
}
