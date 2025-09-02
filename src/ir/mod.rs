pub mod config;

pub use config::{
    BackReferenceSpec, ColumnSpec, CompositeTypeFieldSpec, CompositeTypeSpec, Config, DomainSpec,
    EnumSpec, ExtensionSpec, ForeignKeySpec, FunctionSpec, GrantSpec, IndexSpec,
    MaterializedViewSpec, OutputSpec, PolicySpec, PrimaryKeySpec, RoleSpec, SchemaSpec,
    SequenceSpec, TableSpec, TestSpec, TriggerSpec, ViewSpec,
};
