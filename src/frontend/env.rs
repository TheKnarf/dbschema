use std::collections::HashMap;

/// Variables available during expression evaluation.
///
/// The evaluation engine exposes several namespaces:
/// - `var.<name>` for values in [`Self::vars`]
/// - `local.<name>` or `locals.<name>` for values in [`Self::locals`]
/// - `module.<name>.<output>` for outputs produced by modules
/// - `each.key`/`each.value` inside `for_each` blocks
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
/// };
/// // `local.name` resolves to "bob" while `var.name` resolves to "world".
/// ```
#[derive(Default, Clone, Debug)]
pub struct EnvVars {
    /// Variables passed from the outside world, resolved as `var.*`.
    pub vars: HashMap<String, hcl::Value>,
    /// Locally defined values, resolved as `local.*` or `locals.*`.
    pub locals: HashMap<String, hcl::Value>,
    /// Outputs from loaded modules, referenced as `module.<name>.<output>`.
    pub modules: HashMap<String, HashMap<String, hcl::Value>>,
    /// Key/value for the current iteration of a `for_each` block, enabling `each.key` and `each.value`.
    pub each: Option<(hcl::Value, hcl::Value)>, // (key, value)
}
