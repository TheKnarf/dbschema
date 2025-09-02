use crate::frontend::ast;
use crate::ir;

pub fn lower_config(ast: ast::Config) -> ir::Config {
    ir::Config {
        functions: ast.functions.into_iter().map(Into::into).collect(),
        triggers: ast.triggers.into_iter().map(Into::into).collect(),
        extensions: ast.extensions.into_iter().map(Into::into).collect(),
        schemas: ast.schemas.into_iter().map(Into::into).collect(),
        enums: ast.enums.into_iter().map(Into::into).collect(),
        tables: ast.tables.into_iter().map(Into::into).collect(),
        views: ast.views.into_iter().map(Into::into).collect(),
        materialized: ast.materialized.into_iter().map(Into::into).collect(),
        policies: ast.policies.into_iter().map(Into::into).collect(),
        roles: ast.roles.into_iter().map(Into::into).collect(),
        grants: ast.grants.into_iter().map(Into::into).collect(),
        tests: ast.tests.into_iter().map(Into::into).collect(),
        outputs: ast.outputs.into_iter().map(Into::into).collect(),
    }
}

impl From<ast::AstFunction> for ir::FunctionSpec {
    fn from(f: ast::AstFunction) -> Self {
        Self {
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
}

impl From<ast::AstTrigger> for ir::TriggerSpec {
    fn from(t: ast::AstTrigger) -> Self {
        Self {
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
}

impl From<ast::AstExtension> for ir::ExtensionSpec {
    fn from(e: ast::AstExtension) -> Self {
        Self {
            name: e.name,
            alt_name: e.alt_name,
            if_not_exists: e.if_not_exists,
            schema: e.schema,
            version: e.version,
        }
    }
}

impl From<ast::AstSchema> for ir::SchemaSpec {
    fn from(s: ast::AstSchema) -> Self {
        Self {
            name: s.name,
            alt_name: s.alt_name,
            if_not_exists: s.if_not_exists,
            authorization: s.authorization,
        }
    }
}

impl From<ast::AstEnum> for ir::EnumSpec {
    fn from(e: ast::AstEnum) -> Self {
        Self {
            name: e.name,
            alt_name: e.alt_name,
            schema: e.schema,
            values: e.values,
        }
    }
}

impl From<ast::AstView> for ir::ViewSpec {
    fn from(v: ast::AstView) -> Self {
        Self {
            name: v.name,
            alt_name: v.alt_name,
            schema: v.schema,
            replace: v.replace,
            sql: v.sql,
        }
    }
}

impl From<ast::AstMaterializedView> for ir::MaterializedViewSpec {
    fn from(m: ast::AstMaterializedView) -> Self {
        Self {
            name: m.name,
            alt_name: m.alt_name,
            schema: m.schema,
            with_data: m.with_data,
            sql: m.sql,
        }
    }
}

impl From<ast::AstPolicy> for ir::PolicySpec {
    fn from(p: ast::AstPolicy) -> Self {
        Self {
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
}

impl From<ast::AstRole> for ir::RoleSpec {
    fn from(r: ast::AstRole) -> Self {
        Self {
            name: r.name,
            alt_name: r.alt_name,
            login: r.login,
        }
    }
}

impl From<ast::AstGrant> for ir::GrantSpec {
    fn from(g: ast::AstGrant) -> Self {
        Self {
            name: g.name,
            role: g.role,
            privileges: g.privileges,
            schema: g.schema,
            table: g.table,
            function: g.function,
        }
    }
}

impl From<ast::AstTable> for ir::TableSpec {
    fn from(t: ast::AstTable) -> Self {
        Self {
            name: t.name,
            table_name: t.table_name,
            schema: t.schema,
            if_not_exists: t.if_not_exists,
            columns: t.columns.into_iter().map(Into::into).collect(),
            primary_key: t.primary_key.map(Into::into),
            indexes: t.indexes.into_iter().map(Into::into).collect(),
            foreign_keys: t.foreign_keys.into_iter().map(Into::into).collect(),
            back_references: t.back_references.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ast::AstColumn> for ir::ColumnSpec {
    fn from(c: ast::AstColumn) -> Self {
        Self {
            name: c.name,
            r#type: c.r#type,
            nullable: c.nullable,
            default: c.default,
            db_type: c.db_type,
        }
    }
}

impl From<ast::AstPrimaryKey> for ir::PrimaryKeySpec {
    fn from(pk: ast::AstPrimaryKey) -> Self {
        Self {
            name: pk.name,
            columns: pk.columns,
        }
    }
}

impl From<ast::AstIndex> for ir::IndexSpec {
    fn from(i: ast::AstIndex) -> Self {
        Self {
            name: i.name,
            columns: i.columns,
            unique: i.unique,
        }
    }
}

impl From<ast::AstForeignKey> for ir::ForeignKeySpec {
    fn from(fk: ast::AstForeignKey) -> Self {
        Self {
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
}

impl From<ast::AstBackReference> for ir::BackReferenceSpec {
    fn from(br: ast::AstBackReference) -> Self {
        Self {
            name: br.name,
            table: br.table,
        }
    }
}

impl From<ast::AstTest> for ir::TestSpec {
    fn from(t: ast::AstTest) -> Self {
        Self {
            name: t.name,
            setup: t.setup,
            assert_sql: t.assert_sql,
            teardown: t.teardown,
        }
    }
}

impl From<ast::AstOutput> for ir::OutputSpec {
    fn from(o: ast::AstOutput) -> Self {
        Self {
            name: o.name,
            value: o.value,
        }
    }
}
