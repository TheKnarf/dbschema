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
    pub schema: Option<String>,
    pub table: String,
    pub timing: String,         // BEFORE | AFTER
    pub events: Vec<String>,    // INSERT | UPDATE | DELETE
    pub level: String,          // ROW | STATEMENT
    pub function: String,       // function name (unqualified)
    pub function_schema: Option<String>,
    pub when: Option<String>,   // optional condition, raw SQL
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtensionSpec {
    pub name: String,
    pub if_not_exists: bool,
    pub schema: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaSpec {
    pub name: String,
    pub if_not_exists: bool,
    pub authorization: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnumSpec {
    pub name: String,
    pub schema: Option<String>,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewSpec {
    pub name: String,
    pub schema: Option<String>,
    pub replace: bool, // OR REPLACE
    pub sql: String,   // SELECT ... body
}

#[derive(Debug, Clone, Serialize)]
pub struct MaterializedViewSpec {
    pub name: String,
    pub schema: Option<String>,
    pub with_data: bool, // WITH [NO] DATA
    pub sql: String,     // SELECT ... body
}

#[derive(Debug, Clone, Serialize)]
pub struct PolicySpec {
    pub name: String,
    pub schema: Option<String>,
    pub table: String,
    pub command: String,           // ALL | SELECT | INSERT | UPDATE | DELETE
    pub r#as: Option<String>,      // PERMISSIVE | RESTRICTIVE
    pub roles: Vec<String>,        // empty => PUBLIC (omit TO clause)
    pub using: Option<String>,     // USING (expr)
    pub check: Option<String>,     // WITH CHECK (expr)
}

#[derive(Debug, Clone, Serialize)]
pub struct TableSpec {
    pub name: String,
    pub schema: Option<String>,
    pub if_not_exists: bool,
    pub columns: Vec<ColumnSpec>,
    pub primary_key: Option<PrimaryKeySpec>,
    pub indexes: Vec<IndexSpec>,
    pub foreign_keys: Vec<ForeignKeySpec>,
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
}

#[derive(Debug, Clone, Serialize)]
pub struct TestSpec {
    pub name: String,
    pub setup: Vec<String>,
    pub assert_sql: String,
    pub teardown: Vec<String>,
}

#[derive(Default, Clone, Debug)]
pub struct EnvVars {
    pub vars: HashMap<String, hcl::Value>,
    pub locals: HashMap<String, hcl::Value>,
    pub each: Option<(hcl::Value, hcl::Value)>, // (key, value)
}