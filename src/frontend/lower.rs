use crate::frontend::ast;
use crate::ir;

pub fn lower_config(ast: ast::Config) -> ir::Config {
    let mut providers: Vec<ir::ProviderSpec> = ast.providers.into_iter().map(Into::into).collect();
    if providers.is_empty() {
        providers.push(ir::ProviderSpec {
            provider_type: "postgres".to_string(),
            version: None,
        });
    }

    ir::Config {
        providers,
        functions: ast.functions.into_iter().map(Into::into).collect(),
        procedures: ast.procedures.into_iter().map(Into::into).collect(),
        aggregates: ast.aggregates.into_iter().map(Into::into).collect(),
        operators: ast.operators.into_iter().map(Into::into).collect(),
        triggers: ast.triggers.into_iter().map(Into::into).collect(),
        rules: ast.rules.into_iter().map(Into::into).collect(),
        event_triggers: ast.event_triggers.into_iter().map(Into::into).collect(),
        extensions: ast.extensions.into_iter().map(Into::into).collect(),
        collations: ast.collations.into_iter().map(Into::into).collect(),
        sequences: ast.sequences.into_iter().map(Into::into).collect(),
        schemas: ast.schemas.into_iter().map(Into::into).collect(),
        enums: ast.enums.into_iter().map(Into::into).collect(),
        domains: ast.domains.into_iter().map(Into::into).collect(),
        types: ast.types.into_iter().map(Into::into).collect(),
        tables: ast.tables.into_iter().map(Into::into).collect(),
        indexes: ast.indexes.into_iter().map(Into::into).collect(),
        statistics: ast.statistics.into_iter().map(Into::into).collect(),
        views: ast.views.into_iter().map(Into::into).collect(),
        materialized: ast.materialized.into_iter().map(Into::into).collect(),
        policies: ast.policies.into_iter().map(Into::into).collect(),
        roles: ast.roles.into_iter().map(Into::into).collect(),
        tablespaces: ast.tablespaces.into_iter().map(Into::into).collect(),
        grants: ast.grants.into_iter().map(Into::into).collect(),
        foreign_data_wrappers: ast
            .foreign_data_wrappers
            .into_iter()
            .map(Into::into)
            .collect(),
        foreign_servers: ast.foreign_servers.into_iter().map(Into::into).collect(),
        foreign_tables: ast.foreign_tables.into_iter().map(Into::into).collect(),
        text_search_dictionaries: ast
            .text_search_dictionaries
            .into_iter()
            .map(Into::into)
            .collect(),
        text_search_configurations: ast
            .text_search_configurations
            .into_iter()
            .map(Into::into)
            .collect(),
        text_search_templates: ast
            .text_search_templates
            .into_iter()
            .map(Into::into)
            .collect(),
        text_search_parsers: ast
            .text_search_parsers
            .into_iter()
            .map(Into::into)
            .collect(),
        publications: ast.publications.into_iter().map(Into::into).collect(),
        subscriptions: ast.subscriptions.into_iter().map(Into::into).collect(),
        tests: ast.tests.into_iter().map(Into::into).collect(),
        outputs: ast.outputs.into_iter().map(Into::into).collect(),
    }
}

impl From<ast::AstProvider> for ir::ProviderSpec {
    fn from(provider: ast::AstProvider) -> Self {
        Self {
            provider_type: provider.provider_type,
            version: provider.version,
        }
    }
}

impl From<ast::AstFunction> for ir::FunctionSpec {
    fn from(f: ast::AstFunction) -> Self {
        Self {
            name: f.name,
            alt_name: f.alt_name,
            schema: f.schema,
            language: f.language,
            parameters: f.parameters,
            returns: f.returns,
            replace: f.replace,
            volatility: f.volatility,
            strict: f.strict,
            security: f.security,
            cost: f.cost,
            body: f.body,
            comment: f.comment,
        }
    }
}

impl From<ast::AstProcedure> for ir::ProcedureSpec {
    fn from(p: ast::AstProcedure) -> Self {
        Self {
            name: p.name,
            alt_name: p.alt_name,
            schema: p.schema,
            language: p.language,
            parameters: p.parameters,
            replace: p.replace,
            security: p.security,
            body: p.body,
            comment: p.comment,
        }
    }
}

impl From<ast::AstAggregate> for ir::AggregateSpec {
    fn from(a: ast::AstAggregate) -> Self {
        Self {
            name: a.name,
            alt_name: a.alt_name,
            schema: a.schema,
            inputs: a.inputs,
            sfunc: a.sfunc,
            stype: a.stype,
            finalfunc: a.finalfunc,
            initcond: a.initcond,
            parallel: a.parallel,
            comment: a.comment,
        }
    }
}

impl From<ast::AstOperator> for ir::OperatorSpec {
    fn from(o: ast::AstOperator) -> Self {
        Self {
            name: o.name,
            alt_name: o.alt_name,
            schema: o.schema,
            left: o.left,
            right: o.right,
            procedure: o.procedure,
            commutator: o.commutator,
            negator: o.negator,
            restrict: o.restrict,
            join: o.join,
            comment: o.comment,
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
            comment: t.comment,
        }
    }
}

impl From<ast::AstRule> for ir::RuleSpec {
    fn from(r: ast::AstRule) -> Self {
        Self {
            name: r.name,
            alt_name: r.alt_name,
            schema: r.schema,
            table: r.table,
            event: r.event,
            r#where: r.r#where,
            instead: r.instead,
            command: r.command,
            comment: r.comment,
        }
    }
}

impl From<ast::AstEventTrigger> for ir::EventTriggerSpec {
    fn from(t: ast::AstEventTrigger) -> Self {
        Self {
            name: t.name,
            alt_name: t.alt_name,
            event: t.event,
            tags: t.tags,
            function: t.function,
            function_schema: t.function_schema,
            comment: t.comment,
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
            comment: e.comment,
        }
    }
}

impl From<ast::AstCollation> for ir::CollationSpec {
    fn from(c: ast::AstCollation) -> Self {
        Self {
            name: c.name,
            alt_name: c.alt_name,
            schema: c.schema,
            if_not_exists: c.if_not_exists,
            from: c.from,
            locale: c.locale,
            lc_collate: c.lc_collate,
            lc_ctype: c.lc_ctype,
            provider: c.provider,
            deterministic: c.deterministic,
            version: c.version,
            comment: c.comment,
        }
    }
}

impl From<ast::AstSequence> for ir::SequenceSpec {
    fn from(s: ast::AstSequence) -> Self {
        Self {
            name: s.name,
            alt_name: s.alt_name,
            schema: s.schema,
            if_not_exists: s.if_not_exists,
            r#as: s.r#as,
            increment: s.increment,
            min_value: s.min_value,
            max_value: s.max_value,
            start: s.start,
            cache: s.cache,
            cycle: s.cycle,
            owned_by: s.owned_by,
            comment: s.comment,
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
            comment: s.comment,
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
            comment: e.comment,
        }
    }
}

impl From<ast::AstDomain> for ir::DomainSpec {
    fn from(d: ast::AstDomain) -> Self {
        Self {
            name: d.name,
            alt_name: d.alt_name,
            schema: d.schema,
            r#type: d.r#type,
            not_null: d.not_null,
            default: d.default,
            constraint: d.constraint,
            check: d.check,
            comment: d.comment,
        }
    }
}

impl From<ast::AstCompositeType> for ir::CompositeTypeSpec {
    fn from(t: ast::AstCompositeType) -> Self {
        Self {
            name: t.name,
            alt_name: t.alt_name,
            schema: t.schema,
            fields: t.fields.into_iter().map(Into::into).collect(),
            comment: t.comment,
        }
    }
}

impl From<ast::AstCompositeTypeField> for ir::CompositeTypeFieldSpec {
    fn from(f: ast::AstCompositeTypeField) -> Self {
        Self {
            name: f.name,
            r#type: f.r#type,
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
            comment: v.comment,
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
            comment: m.comment,
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
            comment: p.comment,
        }
    }
}

impl From<ast::AstRole> for ir::RoleSpec {
    fn from(r: ast::AstRole) -> Self {
        Self {
            name: r.name,
            alt_name: r.alt_name,
            login: r.login,
            superuser: r.superuser,
            createdb: r.createdb,
            createrole: r.createrole,
            replication: r.replication,
            password: r.password,
            in_role: r.in_role,
            comment: r.comment,
        }
    }
}

impl From<ast::AstTablespace> for ir::TablespaceSpec {
    fn from(t: ast::AstTablespace) -> Self {
        Self {
            name: t.name,
            alt_name: t.alt_name,
            location: t.location,
            owner: t.owner,
            options: t.options,
            comment: t.comment,
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
            database: g.database,
            sequence: g.sequence,
        }
    }
}

impl From<ast::AstForeignDataWrapper> for ir::ForeignDataWrapperSpec {
    fn from(f: ast::AstForeignDataWrapper) -> Self {
        Self {
            name: f.name,
            alt_name: f.alt_name,
            handler: f.handler,
            validator: f.validator,
            options: f.options,
            comment: f.comment,
        }
    }
}

impl From<ast::AstForeignServer> for ir::ForeignServerSpec {
    fn from(s: ast::AstForeignServer) -> Self {
        Self {
            name: s.name,
            alt_name: s.alt_name,
            wrapper: s.wrapper,
            r#type: s.r#type,
            version: s.version,
            options: s.options,
            comment: s.comment,
        }
    }
}

impl From<ast::AstForeignTable> for ir::ForeignTableSpec {
    fn from(t: ast::AstForeignTable) -> Self {
        Self {
            name: t.name,
            alt_name: t.alt_name,
            schema: t.schema,
            server: t.server,
            columns: t.columns.into_iter().map(Into::into).collect(),
            options: t.options,
            comment: t.comment,
        }
    }
}

impl From<ast::AstTextSearchDictionary> for ir::TextSearchDictionarySpec {
    fn from(d: ast::AstTextSearchDictionary) -> Self {
        Self {
            name: d.name,
            alt_name: d.alt_name,
            schema: d.schema,
            template: d.template,
            options: d.options,
            comment: d.comment,
        }
    }
}

impl From<ast::AstTextSearchConfigurationMapping> for ir::TextSearchConfigurationMappingSpec {
    fn from(m: ast::AstTextSearchConfigurationMapping) -> Self {
        Self {
            tokens: m.tokens,
            dictionaries: m.dictionaries,
        }
    }
}

impl From<ast::AstTextSearchConfiguration> for ir::TextSearchConfigurationSpec {
    fn from(c: ast::AstTextSearchConfiguration) -> Self {
        Self {
            name: c.name,
            alt_name: c.alt_name,
            schema: c.schema,
            parser: c.parser,
            mappings: c.mappings.into_iter().map(Into::into).collect(),
            comment: c.comment,
        }
    }
}

impl From<ast::AstTextSearchTemplate> for ir::TextSearchTemplateSpec {
    fn from(t: ast::AstTextSearchTemplate) -> Self {
        Self {
            name: t.name,
            alt_name: t.alt_name,
            schema: t.schema,
            init: t.init,
            lexize: t.lexize,
            comment: t.comment,
        }
    }
}

impl From<ast::AstTextSearchParser> for ir::TextSearchParserSpec {
    fn from(p: ast::AstTextSearchParser) -> Self {
        Self {
            name: p.name,
            alt_name: p.alt_name,
            schema: p.schema,
            start: p.start,
            gettoken: p.gettoken,
            end: p.end,
            headline: p.headline,
            lextypes: p.lextypes,
            comment: p.comment,
        }
    }
}

impl From<ast::AstTable> for ir::TableSpec {
    fn from(t: ast::AstTable) -> Self {
        Self {
            name: t.name,
            alt_name: t.alt_name,
            schema: t.schema,
            if_not_exists: t.if_not_exists,
            columns: t.columns.into_iter().map(Into::into).collect(),
            primary_key: t.primary_key.map(Into::into),
            indexes: t.indexes.into_iter().map(Into::into).collect(),
            checks: t.checks.into_iter().map(Into::into).collect(),
            foreign_keys: t.foreign_keys.into_iter().map(Into::into).collect(),
            partition_by: t.partition_by.map(Into::into),
            partitions: t.partitions.into_iter().map(Into::into).collect(),
            back_references: t.back_references.into_iter().map(Into::into).collect(),
            lint_ignore: t.lint_ignore,
            comment: t.comment,
            map: t.map,
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
            lint_ignore: c.lint_ignore,
            comment: c.comment,
            count: c.count,
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
            expressions: i.expressions,
            r#where: i.r#where,
            orders: i.orders,
            operator_classes: i.operator_classes,
            unique: i.unique,
        }
    }
}

impl From<ast::AstCheck> for ir::CheckSpec {
    fn from(c: ast::AstCheck) -> Self {
        Self {
            name: c.name,
            expression: c.expression,
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

impl From<ast::AstPartitionBy> for ir::PartitionBySpec {
    fn from(p: ast::AstPartitionBy) -> Self {
        Self {
            strategy: p.strategy,
            columns: p.columns,
        }
    }
}

impl From<ast::AstPartition> for ir::PartitionSpec {
    fn from(p: ast::AstPartition) -> Self {
        Self {
            name: p.name,
            values: p.values,
        }
    }
}

impl From<ast::AstStandaloneIndex> for ir::StandaloneIndexSpec {
    fn from(i: ast::AstStandaloneIndex) -> Self {
        Self {
            name: i.name,
            table: i.table,
            schema: i.schema,
            columns: i.columns,
            expressions: i.expressions,
            r#where: i.r#where,
            orders: i.orders,
            operator_classes: i.operator_classes,
            unique: i.unique,
        }
    }
}

impl From<ast::AstStatistics> for ir::StatisticsSpec {
    fn from(s: ast::AstStatistics) -> Self {
        Self {
            name: s.name,
            alt_name: s.alt_name,
            schema: s.schema,
            table: s.table,
            columns: s.columns,
            kinds: s.kinds,
            comment: s.comment,
        }
    }
}

impl From<ast::AstPublication> for ir::PublicationSpec {
    fn from(p: ast::AstPublication) -> Self {
        Self {
            name: p.name,
            alt_name: p.alt_name,
            all_tables: p.all_tables,
            tables: p.tables.into_iter().map(Into::into).collect(),
            publish: p.publish,
            comment: p.comment,
        }
    }
}

impl From<ast::AstPublicationTable> for ir::PublicationTableSpec {
    fn from(t: ast::AstPublicationTable) -> Self {
        Self {
            schema: t.schema,
            table: t.table,
        }
    }
}

impl From<ast::AstSubscription> for ir::SubscriptionSpec {
    fn from(s: ast::AstSubscription) -> Self {
        Self {
            name: s.name,
            alt_name: s.alt_name,
            connection: s.connection,
            publications: s.publications,
            comment: s.comment,
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
            asserts: t.asserts,
            assert_fail: t.assert_fail,
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
