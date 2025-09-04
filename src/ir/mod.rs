pub mod config;

pub use config::{
    AggregateSpec, BackReferenceSpec, CheckSpec, ColumnSpec, CompositeTypeFieldSpec,
    CompositeTypeSpec, Config, DomainSpec, EnumSpec, EventTriggerSpec, ExtensionSpec,
    ForeignKeySpec, FunctionSpec, GrantSpec, IndexSpec, MaterializedViewSpec, OutputSpec,
    PartitionBySpec, PartitionSpec, PolicySpec, PrimaryKeySpec, RoleSpec, SchemaSpec, SequenceSpec,
    StandaloneIndexSpec, TableSpec, TestSpec, TriggerSpec, ViewSpec,
};
