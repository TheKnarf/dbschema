use crate::frontend::ast;
use crate::ir;

pub fn lower_config(ast: ast::Config) -> ir::Config {
    ir::Config {
        functions: ast.functions.into_iter().map(lower_function).collect(),
        triggers: ast.triggers.into_iter().map(lower_trigger).collect(),
        extensions: ast.extensions.into_iter().map(lower_extension).collect(),
        schemas: ast.schemas.into_iter().map(lower_schema).collect(),
        enums: ast.enums.into_iter().map(lower_enum).collect(),
        tables: ast.tables.into_iter().map(lower_table).collect(),
        views: ast.views.into_iter().map(lower_view).collect(),
        materialized: ast
            .materialized
            .into_iter()
            .map(lower_materialized)
            .collect(),
        policies: ast.policies.into_iter().map(lower_policy).collect(),
        tests: ast.tests.into_iter().map(lower_test).collect(),
    }
}

fn lower_function(f: ast::AstFunction) -> ir::FunctionSpec {
    ir::FunctionSpec {
        name: f.name,
        alt_name: f.alt_name,
        schema: f.schema,
        language: f.language,
        returns: f.returns,
        replace: f.replace,
        security_definer: f.security_definer,
        body: f.body,
    }
}

fn lower_trigger(t: ast::AstTrigger) -> ir::TriggerSpec {
    ir::TriggerSpec {
        name: t.name,
        alt_name: t.alt_name,
        schema: t.schema,
        table: t.table,
        timing: t.timing,
        events: t.events,
        level: t.level,
        function: t.function,
        function_schema: t.function_schema,
        when: t.when,
    }
}

fn lower_extension(e: ast::AstExtension) -> ir::ExtensionSpec {
    ir::ExtensionSpec {
        name: e.name,
        alt_name: e.alt_name,
        if_not_exists: e.if_not_exists,
        schema: e.schema,
        version: e.version,
    }
}

fn lower_schema(s: ast::AstSchema) -> ir::SchemaSpec {
    ir::SchemaSpec {
        name: s.name,
        alt_name: s.alt_name,
        if_not_exists: s.if_not_exists,
        authorization: s.authorization,
    }
}

fn lower_enum(e: ast::AstEnum) -> ir::EnumSpec {
    ir::EnumSpec {
        name: e.name,
        alt_name: e.alt_name,
        schema: e.schema,
        values: e.values,
    }
}

fn lower_view(v: ast::AstView) -> ir::ViewSpec {
    ir::ViewSpec {
        name: v.name,
        alt_name: v.alt_name,
        schema: v.schema,
        replace: v.replace,
        sql: v.sql,
    }
}

fn lower_materialized(m: ast::AstMaterializedView) -> ir::MaterializedViewSpec {
    ir::MaterializedViewSpec {
        name: m.name,
        alt_name: m.alt_name,
        schema: m.schema,
        with_data: m.with_data,
        sql: m.sql,
    }
}

fn lower_policy(p: ast::AstPolicy) -> ir::PolicySpec {
    ir::PolicySpec {
        name: p.name,
        alt_name: p.alt_name,
        schema: p.schema,
        table: p.table,
        command: p.command,
        r#as: p.r#as,
        roles: p.roles,
        using: p.using,
        check: p.check,
    }
}

fn lower_table(t: ast::AstTable) -> ir::TableSpec {
    ir::TableSpec {
        name: t.name,
        table_name: t.table_name,
        schema: t.schema,
        if_not_exists: t.if_not_exists,
        columns: t.columns.into_iter().map(lower_column).collect(),
        primary_key: t.primary_key.map(lower_primary_key),
        indexes: t.indexes.into_iter().map(lower_index).collect(),
        foreign_keys: t.foreign_keys.into_iter().map(lower_foreign_key).collect(),
        back_references: t
            .back_references
            .into_iter()
            .map(lower_back_reference)
            .collect(),
    }
}

fn lower_column(c: ast::AstColumn) -> ir::ColumnSpec {
    ir::ColumnSpec {
        name: c.name,
        r#type: c.r#type,
        nullable: c.nullable,
        default: c.default,
        db_type: c.db_type,
    }
}

fn lower_primary_key(pk: ast::AstPrimaryKey) -> ir::PrimaryKeySpec {
    ir::PrimaryKeySpec {
        name: pk.name,
        columns: pk.columns,
    }
}

fn lower_index(i: ast::AstIndex) -> ir::IndexSpec {
    ir::IndexSpec {
        name: i.name,
        columns: i.columns,
        unique: i.unique,
    }
}

fn lower_foreign_key(fk: ast::AstForeignKey) -> ir::ForeignKeySpec {
    ir::ForeignKeySpec {
        name: fk.name,
        columns: fk.columns,
        ref_schema: fk.ref_schema,
        ref_table: fk.ref_table,
        ref_columns: fk.ref_columns,
        on_delete: fk.on_delete,
        on_update: fk.on_update,
        back_reference_name: fk.back_reference_name,
    }
}

fn lower_back_reference(br: ast::AstBackReference) -> ir::BackReferenceSpec {
    ir::BackReferenceSpec {
        name: br.name,
        table: br.table,
    }
}

fn lower_test(t: ast::AstTest) -> ir::TestSpec {
    ir::TestSpec {
        name: t.name,
        setup: t.setup,
        assert_sql: t.assert_sql,
        teardown: t.teardown,
    }
}
