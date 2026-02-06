pub mod config;

pub use config::{
    AggregateSpec, BackReferenceSpec, CheckSpec, CollationSpec, ColumnSpec, CompositeTypeFieldSpec,
    CompositeTypeSpec, Config, DomainSpec, EnumSpec, EqAssertSpec, ErrorAssertSpec,
    EventTriggerSpec, ExtensionSpec,
    ForeignDataWrapperSpec, ForeignKeySpec, ForeignServerSpec, ForeignTableSpec, FunctionSpec,
    GrantSpec, IndexSpec, MaterializedViewSpec, NotifyAssertSpec, OperatorSpec, OutputSpec, PartitionBySpec,
    PartitionSpec, PolicySpec, PrimaryKeySpec, ProcedureSpec, ProviderSpec, PublicationSpec,
    PublicationTableSpec, RoleSpec, RuleSpec, SchemaSpec, SequenceSpec, StandaloneIndexSpec,
    StatisticsSpec, SubscriptionSpec, TableSpec, TablespaceSpec, TestSpec,
    TextSearchConfigurationMappingSpec, TextSearchConfigurationSpec, TextSearchDictionarySpec,
    TextSearchParserSpec, TextSearchTemplateSpec, TriggerSpec, ViewSpec,
};
