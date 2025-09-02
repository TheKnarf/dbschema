use std::collections::HashMap;

/// Variables available during expression evaluation.
///
/// The evaluation engine exposes three namespaces:
/// - `var.<name>` for values in [`Self::vars`]
/// - `local.<name>` or `locals.<name>` for values in [`Self::locals`]
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
    /// Key/value for the current iteration of a `for_each` block, enabling `each.key` and `each.value`.
    pub each: Option<(hcl::Value, hcl::Value)>, // (key, value)
}
