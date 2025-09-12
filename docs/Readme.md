# Documentation

## Features

### Postgres resources

- [schema](postgres/schema.md)
- [enum](postgres/enum.md)
- [domain](postgres/domain.md)
- [type](postgres/type.md)
- [sequence](postgres/sequence.md)
- [table](postgres/table.md)
- [index](postgres/index.md)
- [view](postgres/view.md)
- [materialized view](postgres/materialized.md)
- [function](postgres/function.md)
- [aggregate](postgres/aggregate.md)
- [trigger](postgres/trigger.md)
- [event trigger](postgres/event_trigger.md)
- [extension](postgres/extension.md)
- [policy](postgres/policy.md)
- [role](postgres/role.md)
- [grant](postgres/grant.md)
- [publication](postgres/publication.md)
- [subscription](postgres/subscription.md)

### Generic HCL

- [variables and repetition](variables.md)
- [locals](locals.md)
- [modules](modules.md)
- [output](output.md)
- [tests](tests.md)

- Variables, locals, modules, output, and tests are supported via a small HCL dialect.
- Validate config, then generate SQL with safe `CREATE OR REPLACE FUNCTION` and idempotent guards for triggers/enums/materialized views.

## Guides

- [Configuration](configuration.md)
- [Linting](linting.md)

## Index of documents

- Generic
  - [Configuration](configuration.md)
  - [Linting](linting.md)
  - [Variables and Repetition](variables.md)
  - [Locals](locals.md)
  - [Modules](modules.md)
  - [Output](output.md)
  - [Tests](tests.md)
- Postgres
  - [Aggregate](postgres/aggregate.md)
  - [Domain](postgres/domain.md)
  - [Enum](postgres/enum.md)
  - [Event Trigger](postgres/event_trigger.md)
  - [Extension](postgres/extension.md)
  - [Function](postgres/function.md)
  - [Grant](postgres/grant.md)
  - [Index](postgres/index.md)
  - [Materialized View](postgres/materialized.md)
  - [Policy](postgres/policy.md)
  - [Publication](postgres/publication.md)
  - [Role](postgres/role.md)
  - [Schema](postgres/schema.md)
  - [Sequence](postgres/sequence.md)
  - [Subscription](postgres/subscription.md)
  - [Table](postgres/table.md)
  - [Trigger](postgres/trigger.md)
  - [Type](postgres/type.md)
  - [View](postgres/view.md)
