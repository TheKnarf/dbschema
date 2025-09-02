use hcl::Value;

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub functions: Vec<AstFunction>,
    pub triggers: Vec<AstTrigger>,
    pub extensions: Vec<AstExtension>,
    pub sequences: Vec<AstSequence>,
    pub schemas: Vec<AstSchema>,
    pub enums: Vec<AstEnum>,
    pub domains: Vec<AstDomain>,
    pub types: Vec<AstCompositeType>,
    pub tables: Vec<AstTable>,
    pub views: Vec<AstView>,
    pub materialized: Vec<AstMaterializedView>,
    pub policies: Vec<AstPolicy>,
    pub roles: Vec<AstRole>,
    pub grants: Vec<AstGrant>,
    pub tests: Vec<AstTest>,
    pub outputs: Vec<AstOutput>,
}

#[derive(Debug, Clone)]
pub struct AstFunction {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub language: String,
    pub returns: String,
    pub replace: bool,
    pub security_definer: bool,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct AstTrigger {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub table: String,
    pub timing: String,
    pub events: Vec<String>,
    pub level: String,
    pub function: String,
    pub function_schema: Option<String>,
    pub when: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AstExtension {
    pub name: String,
    pub alt_name: Option<String>,
    pub if_not_exists: bool,
    pub schema: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AstSequence {
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
}

#[derive(Debug, Clone)]
pub struct AstSchema {
    pub name: String,
    pub alt_name: Option<String>,
    pub if_not_exists: bool,
    pub authorization: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AstEnum {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub values: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AstDomain {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub r#type: String,
    pub not_null: bool,
    pub default: Option<String>,
    pub constraint: Option<String>,
    pub check: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AstCompositeType {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub fields: Vec<AstCompositeTypeField>,
}

#[derive(Debug, Clone)]
pub struct AstCompositeTypeField {
    pub name: String,
    pub r#type: String,
}

#[derive(Debug, Clone)]
pub struct AstView {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub replace: bool,
    pub sql: String,
}

#[derive(Debug, Clone)]
pub struct AstMaterializedView {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub with_data: bool,
    pub sql: String,
}

#[derive(Debug, Clone)]
pub struct AstPolicy {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub table: String,
    pub command: String,
    pub r#as: Option<String>,
    pub roles: Vec<String>,
    pub using: Option<String>,
    pub check: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AstRole {
    pub name: String,
    pub alt_name: Option<String>,
    pub login: bool,
}

#[derive(Debug, Clone)]
pub struct AstGrant {
    pub name: String,
    pub role: String,
    pub privileges: Vec<String>,
    pub schema: Option<String>,
    pub table: Option<String>,
    pub function: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AstTable {
    pub name: String,
    pub table_name: Option<String>,
    pub schema: Option<String>,
    pub if_not_exists: bool,
    pub columns: Vec<AstColumn>,
    pub primary_key: Option<AstPrimaryKey>,
    pub indexes: Vec<AstIndex>,
    pub foreign_keys: Vec<AstForeignKey>,
    pub back_references: Vec<AstBackReference>,
    pub lint_ignore: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AstBackReference {
    pub name: String,
    pub table: String,
}

#[derive(Debug, Clone)]
pub struct AstColumn {
    pub name: String,
    pub r#type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub db_type: Option<String>,
    pub lint_ignore: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AstPrimaryKey {
    pub name: Option<String>,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AstIndex {
    pub name: Option<String>,
    pub columns: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone)]
pub struct AstForeignKey {
    pub name: Option<String>,
    pub columns: Vec<String>,
    pub ref_schema: Option<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub on_delete: Option<String>,
    pub on_update: Option<String>,
    pub back_reference_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AstTest {
    pub name: String,
    pub setup: Vec<String>,
    pub assert_sql: String,
    pub teardown: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AstOutput {
    pub name: String,
    pub value: Value,
}
