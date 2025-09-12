pub mod config;

pub use config::{
    AggregateSpec, BackReferenceSpec, CheckSpec, ColumnSpec, CompositeTypeFieldSpec,
    CompositeTypeSpec, Config, DomainSpec, EnumSpec, EventTriggerSpec, ExtensionSpec,
    CollationSpec,
    ForeignDataWrapperSpec, ForeignKeySpec, ForeignServerSpec, ForeignTableSpec,
    FunctionSpec, GrantSpec, IndexSpec, MaterializedViewSpec, OutputSpec,
    PartitionBySpec, PartitionSpec, PolicySpec, PrimaryKeySpec, PublicationSpec,
    PublicationTableSpec, RoleSpec, SchemaSpec, SequenceSpec, StandaloneIndexSpec,
    SubscriptionSpec, TableSpec, TestSpec, TriggerSpec, ViewSpec,
};
