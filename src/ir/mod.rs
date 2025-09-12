pub mod config;

pub use config::{
    AggregateSpec, BackReferenceSpec, CheckSpec, ColumnSpec, CompositeTypeFieldSpec,
    CompositeTypeSpec, Config, DomainSpec, EnumSpec, EventTriggerSpec, ExtensionSpec,
    CollationSpec, OperatorSpec, RuleSpec,
    ForeignDataWrapperSpec, ForeignKeySpec, ForeignServerSpec, ForeignTableSpec,
    FunctionSpec, ProcedureSpec, GrantSpec, IndexSpec, MaterializedViewSpec, OutputSpec,
    PartitionBySpec, PartitionSpec, PolicySpec, PrimaryKeySpec, PublicationSpec,
    PublicationTableSpec, RoleSpec, TablespaceSpec, SchemaSpec, SequenceSpec, StandaloneIndexSpec,
    StatisticsSpec, SubscriptionSpec, TableSpec, TestSpec, TextSearchConfigurationMappingSpec,
    TextSearchConfigurationSpec, TextSearchDictionarySpec, TextSearchParserSpec,
    TextSearchTemplateSpec, TriggerSpec, ViewSpec,
};
