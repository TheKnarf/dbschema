use hcl::Value;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Config {
    pub functions: Vec<FunctionSpec>,
    pub triggers: Vec<TriggerSpec>,
    pub event_triggers: Vec<EventTriggerSpec>,
    pub extensions: Vec<ExtensionSpec>,
    pub sequences: Vec<SequenceSpec>,
    pub schemas: Vec<SchemaSpec>,
    pub enums: Vec<EnumSpec>,
    pub domains: Vec<DomainSpec>,
    pub types: Vec<CompositeTypeSpec>,
    pub tables: Vec<TableSpec>,
    pub indexes: Vec<StandaloneIndexSpec>,
    pub views: Vec<ViewSpec>,
    pub materialized: Vec<MaterializedViewSpec>,
    pub policies: Vec<PolicySpec>,
    pub roles: Vec<RoleSpec>,
    pub grants: Vec<GrantSpec>,
    pub tests: Vec<TestSpec>,
    pub outputs: Vec<OutputSpec>,
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
    pub comment: Option<String>,
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
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventTriggerSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub event: String,
    pub tags: Vec<String>,
    pub function: String,
    pub function_schema: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtensionSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub if_not_exists: bool,
    pub schema: Option<String>,
    pub version: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SequenceSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub if_not_exists: bool,
    pub r#as: Option<String>,
    pub increment: Option<i64>,
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
    pub start: Option<i64>,
    pub cache: Option<i64>,
    pub cycle: bool,
    pub owned_by: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub if_not_exists: bool,
    pub authorization: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnumSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub values: Vec<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DomainSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub r#type: String,
    pub not_null: bool,
    pub default: Option<String>,
    pub constraint: Option<String>,
    pub check: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompositeTypeSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub fields: Vec<CompositeTypeFieldSpec>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompositeTypeFieldSpec {
    pub name: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub replace: bool, // OR REPLACE
    pub sql: String,   // SELECT ... body
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaterializedViewSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub with_data: bool, // WITH [NO] DATA
    pub sql: String,     // SELECT ... body
    pub comment: Option<String>,
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
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoleSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub login: bool,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GrantSpec {
    pub name: String,
    pub role: String,
    pub privileges: Vec<String>,
    pub schema: Option<String>,
    pub table: Option<String>,
    pub function: Option<String>,
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
    pub checks: Vec<CheckSpec>,
    pub foreign_keys: Vec<ForeignKeySpec>,
    pub back_references: Vec<BackReferenceSpec>,
    pub lint_ignore: Vec<String>,
    pub comment: Option<String>,
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
    pub lint_ignore: Vec<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrimaryKeySpec {
    pub name: Option<String>,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckSpec {
    pub name: Option<String>,
    pub expression: String,
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
pub struct StandaloneIndexSpec {
    pub name: String,
    pub table: String,
    pub schema: Option<String>,
    pub columns: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestSpec {
    pub name: String,
    pub setup: Vec<String>,
    pub assert_sql: String,
    pub teardown: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputSpec {
    pub name: String,
    pub value: Value,
}
