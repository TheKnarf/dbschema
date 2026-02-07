# Documentation

dbschema lets you define database schemas in HCL and generate SQL/Prisma artifacts.

## Features

- [Configuration](configuration.md) — Configure `dbschema.toml` targets and global settings.
- [Providers](provider.md) — Declare which database backend to target.
- [Linting](linting.md) — Run built-in checks and tune severities per rule.
- [Variables, Locals, and Repetition](variables.md) — Parameterize HCL, loop with `for_each`/`count`, and use dynamic blocks.
- [Data Sources](data-sources.md) — Load external state (e.g. Prisma schemas) and expose it to your resources.
- [Modules and Output](modules.md) — Reuse HCL modules and return values via `output` blocks.
- [Tests](tests.md) — Define setup/assert SQL and run tests transactionally against Postgres.
- [Scenarios](scenarios.md) — ASP-driven combinatorial testing with clingo.
- [create-migration](create-migration.md) — Generate SQL/Prisma/JSON artifacts from HCL.
- [validate](validate.md) — Validate HCL and summarize discovered resources.
- [fmt](fmt.md) — Format HCL files in place for consistent style.

## Postgres

We aim to support all major PostgreSQL features. See the reference docs:

- [Schema](postgres/schema.md)
- [Enum](postgres/enum.md)
- [Domain](postgres/domain.md)
- [Type](postgres/type.md)
- [Sequence](postgres/sequence.md)
- [Table](postgres/table.md)
- [Index](postgres/index.md)
- [View](postgres/view.md)
- [Materialized View](postgres/materialized.md)
- [Function](postgres/function.md)
- [Procedure](postgres/procedure.md)
- [Aggregate](postgres/aggregate.md)
- [Operator](postgres/operator.md)
- [Trigger](postgres/trigger.md)
- [Rule](postgres/rule.md)
- [Event Trigger](postgres/event_trigger.md)
- [Extension](postgres/extension.md)
- [Collation](postgres/collation.md)
- [Policy](postgres/policy.md)
- [Role](postgres/role.md)
- [Grant](postgres/grant.md)
- [Publication](postgres/publication.md)
- [Subscription](postgres/subscription.md)
- [Foreign Data Wrapper](postgres/foreign_data_wrapper.md)
- [Foreign Server](postgres/foreign_server.md)
- [Foreign Table](postgres/foreign_table.md)
- [Text Search Dictionary](postgres/text_search_dictionary.md)
- [Text Search Configuration](postgres/text_search_configuration.md)
- [Text Search Template](postgres/text_search_template.md)
- [Text Search Parser](postgres/text_search_parser.md)
- [Statistics](postgres/statistics.md)
