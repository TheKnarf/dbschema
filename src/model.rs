use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Config {
    pub functions: Vec<FunctionSpec>,
    pub triggers: Vec<TriggerSpec>,
    pub extensions: Vec<ExtensionSpec>,
    pub schemas: Vec<SchemaSpec>,
    pub enums: Vec<EnumSpec>,
    pub tables: Vec<TableSpec>,
    pub views: Vec<ViewSpec>,
    pub materialized: Vec<MaterializedViewSpec>,
    pub policies: Vec<PolicySpec>,
    pub tests: Vec<TestSpec>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub language: String,
    pub returns: String,
    pub replace: bool,
    pub security_definer: bool,
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TriggerSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub table: String,
    pub timing: String,      // BEFORE | AFTER
    pub events: Vec<String>, // INSERT | UPDATE | DELETE
    pub level: String,       // ROW | STATEMENT
    pub function: String,    // function name (unqualified)
    pub function_schema: Option<String>,
    pub when: Option<String>, // optional condition, raw SQL
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtensionSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub if_not_exists: bool,
    pub schema: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub if_not_exists: bool,
    pub authorization: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnumSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub replace: bool, // OR REPLACE
    pub sql: String,   // SELECT ... body
}

#[derive(Debug, Clone, Serialize)]
pub struct MaterializedViewSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub with_data: bool, // WITH [NO] DATA
    pub sql: String,     // SELECT ... body
}

#[derive(Debug, Clone, Serialize)]
pub struct PolicySpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub table: String,
    pub command: String,       // ALL | SELECT | INSERT | UPDATE | DELETE
    pub r#as: Option<String>,  // PERMISSIVE | RESTRICTIVE
    pub roles: Vec<String>,    // empty => PUBLIC (omit TO clause)
    pub using: Option<String>, // USING (expr)
    pub check: Option<String>, // WITH CHECK (expr)
}

#[derive(Debug, Clone, Serialize)]
pub struct TableSpec {
    pub name: String,
    pub table_name: Option<String>,
    pub schema: Option<String>,
    pub if_not_exists: bool,
    pub columns: Vec<ColumnSpec>,
    pub primary_key: Option<PrimaryKeySpec>,
    pub indexes: Vec<IndexSpec>,
    pub foreign_keys: Vec<ForeignKeySpec>,
    pub back_references: Vec<BackReferenceSpec>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackReferenceSpec {
    pub name: String,
    pub table: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnSpec {
    pub name: String,
    pub r#type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub db_type: Option<String>, // NEW: Database-specific type like "CHAR(32)", "VARCHAR(255)"
}

#[derive(Debug, Clone, Serialize)]
pub struct PrimaryKeySpec {
    pub name: Option<String>,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexSpec {
    pub name: Option<String>,
    pub columns: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForeignKeySpec {
    pub name: Option<String>,
    pub columns: Vec<String>,
    pub ref_schema: Option<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub on_delete: Option<String>,
    pub on_update: Option<String>,
    pub back_reference_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestSpec {
    pub name: String,
    pub setup: Vec<String>,
    pub assert_sql: String,
    pub teardown: Vec<String>,
}

/// Variables available during expression evaluation.
///
/// The evaluation engine exposes three namespaces:
/// - `var.<name>` for values in [`Self::vars`]
/// - `local.<name>` or `locals.<name>` for values in [`Self::locals`]
/// - `each.key`/`each.value` inside `for_each` blocks
///
/// # Example
/// ```
/// use dbschema::model::EnvVars;
/// use hcl::Value;
/// use std::collections::HashMap;
///
/// let env = EnvVars {
///     vars: HashMap::from([("name".into(), Value::from("world"))]),
///     locals: HashMap::from([("name".into(), Value::from("bob"))]),
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
