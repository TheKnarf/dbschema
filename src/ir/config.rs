use hcl::Value;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Config {
    pub providers: Vec<ProviderSpec>,
    pub functions: Vec<FunctionSpec>,
    pub procedures: Vec<ProcedureSpec>,
    pub aggregates: Vec<AggregateSpec>,
    pub operators: Vec<OperatorSpec>,
    pub triggers: Vec<TriggerSpec>,
    pub rules: Vec<RuleSpec>,
    pub event_triggers: Vec<EventTriggerSpec>,
    pub extensions: Vec<ExtensionSpec>,
    pub collations: Vec<CollationSpec>,
    pub sequences: Vec<SequenceSpec>,
    pub schemas: Vec<SchemaSpec>,
    pub enums: Vec<EnumSpec>,
    pub domains: Vec<DomainSpec>,
    pub types: Vec<CompositeTypeSpec>,
    pub tables: Vec<TableSpec>,
    pub indexes: Vec<StandaloneIndexSpec>,
    pub statistics: Vec<StatisticsSpec>,
    pub views: Vec<ViewSpec>,
    pub materialized: Vec<MaterializedViewSpec>,
    pub policies: Vec<PolicySpec>,
    pub roles: Vec<RoleSpec>,
    pub tablespaces: Vec<TablespaceSpec>,
    pub grants: Vec<GrantSpec>,
    pub foreign_data_wrappers: Vec<ForeignDataWrapperSpec>,
    pub foreign_servers: Vec<ForeignServerSpec>,
    pub foreign_tables: Vec<ForeignTableSpec>,
    pub text_search_dictionaries: Vec<TextSearchDictionarySpec>,
    pub text_search_configurations: Vec<TextSearchConfigurationSpec>,
    pub text_search_templates: Vec<TextSearchTemplateSpec>,
    pub text_search_parsers: Vec<TextSearchParserSpec>,
    pub publications: Vec<PublicationSpec>,
    pub subscriptions: Vec<SubscriptionSpec>,
    pub tests: Vec<TestSpec>,
    pub invariants: Vec<InvariantSpec>,
    pub outputs: Vec<OutputSpec>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderSpec {
    pub provider_type: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub language: String,
    pub parameters: Vec<String>,
    pub returns: String,
    pub replace: bool,
    pub volatility: Option<String>,
    pub strict: bool,
    pub security: Option<String>,
    pub cost: Option<f64>,
    pub body: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcedureSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub language: String,
    pub parameters: Vec<String>,
    pub replace: bool,
    pub security: Option<String>,
    pub body: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AggregateSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub inputs: Vec<String>,
    pub sfunc: String,
    pub stype: String,
    pub finalfunc: Option<String>,
    pub initcond: Option<String>,
    pub parallel: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperatorSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub left: Option<String>,
    pub right: Option<String>,
    pub procedure: String,
    pub commutator: Option<String>,
    pub negator: Option<String>,
    pub restrict: Option<String>,
    pub join: Option<String>,
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
pub struct RuleSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub table: String,
    pub event: String,
    pub r#where: Option<String>,
    pub instead: bool,
    pub command: String,
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
pub struct CollationSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub if_not_exists: bool,
    pub from: Option<String>,
    pub locale: Option<String>,
    pub lc_collate: Option<String>,
    pub lc_ctype: Option<String>,
    pub provider: Option<String>,
    pub deterministic: Option<bool>,
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
    pub superuser: bool,
    pub createdb: bool,
    pub createrole: bool,
    pub replication: bool,
    pub password: Option<String>,
    pub in_role: Vec<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TablespaceSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub location: String,
    pub owner: Option<String>,
    pub options: Vec<String>,
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
    pub database: Option<String>,
    pub sequence: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForeignDataWrapperSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub handler: Option<String>,
    pub validator: Option<String>,
    pub options: Vec<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForeignServerSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub wrapper: String,
    pub r#type: Option<String>,
    pub version: Option<String>,
    pub options: Vec<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForeignTableSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub server: String,
    pub columns: Vec<ColumnSpec>,
    pub options: Vec<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicationSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub all_tables: bool,
    pub tables: Vec<PublicationTableSpec>,
    pub publish: Vec<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicationTableSpec {
    pub schema: Option<String>,
    pub table: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub connection: String,
    pub publications: Vec<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub if_not_exists: bool,
    pub columns: Vec<ColumnSpec>,
    pub primary_key: Option<PrimaryKeySpec>,
    pub indexes: Vec<IndexSpec>,
    pub checks: Vec<CheckSpec>,
    pub foreign_keys: Vec<ForeignKeySpec>,
    pub partition_by: Option<PartitionBySpec>,
    pub partitions: Vec<PartitionSpec>,
    pub back_references: Vec<BackReferenceSpec>,
    pub lint_ignore: Vec<String>,
    pub comment: Option<String>,
    pub map: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartitionBySpec {
    pub strategy: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartitionSpec {
    pub name: String,
    pub values: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackReferenceSpec {
    pub name: String,
    pub table: String,
    pub relation_name: Option<String>,
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
    pub count: usize,
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
    pub expressions: Vec<String>,
    pub r#where: Option<String>,
    pub orders: Vec<String>,
    pub operator_classes: Vec<String>,
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
    pub expressions: Vec<String>,
    pub r#where: Option<String>,
    pub orders: Vec<String>,
    pub operator_classes: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatisticsSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub table: String,
    pub columns: Vec<String>,
    pub kinds: Vec<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextSearchDictionarySpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub template: String,
    pub options: Vec<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextSearchConfigurationMappingSpec {
    pub tokens: Vec<String>,
    pub dictionaries: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextSearchConfigurationSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub parser: String,
    pub mappings: Vec<TextSearchConfigurationMappingSpec>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextSearchTemplateSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub init: Option<String>,
    pub lexize: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextSearchParserSpec {
    pub name: String,
    pub alt_name: Option<String>,
    pub schema: Option<String>,
    pub start: String,
    pub gettoken: String,
    pub end: String,
    pub headline: Option<String>,
    pub lextypes: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotifyAssertSpec {
    pub channel: String,
    pub payload_contains: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EqAssertSpec {
    pub query: String,
    pub expected: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorAssertSpec {
    pub sql: String,
    pub message_contains: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapshotAssertSpec {
    pub query: String,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvariantSpec {
    pub name: String,
    pub asserts: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestSpec {
    pub name: String,
    pub setup: Vec<String>,
    pub asserts: Vec<String>,
    pub assert_fail: Vec<String>,
    pub assert_notify: Vec<NotifyAssertSpec>,
    pub assert_eq: Vec<EqAssertSpec>,
    pub assert_error: Vec<ErrorAssertSpec>,
    pub assert_snapshot: Vec<SnapshotAssertSpec>,
    pub teardown: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputSpec {
    pub name: String,
    pub value: Value,
}
