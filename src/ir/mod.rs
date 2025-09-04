pub mod config;

pub use config::{
    BackReferenceSpec, CheckSpec, ColumnSpec, CompositeTypeFieldSpec, CompositeTypeSpec, Config,
    DomainSpec, EnumSpec, EventTriggerSpec, ExtensionSpec, ForeignKeySpec, FunctionSpec,
    AggregateSpec, GrantSpec, IndexSpec, MaterializedViewSpec, OutputSpec, PolicySpec,
    PrimaryKeySpec, RoleSpec, SchemaSpec, SequenceSpec, StandaloneIndexSpec, TableSpec, TestSpec,
    TriggerSpec, ViewSpec,
};
