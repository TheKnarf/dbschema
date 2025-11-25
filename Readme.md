# dbschema

Define database schema's as HCL files, and generate idempotent SQL migrations.
Dbschema aims to support all PostgreSQL features (like extensions, functions, triggers, etc).

Designed to complement Prisma (or any tool) when you want to declaratively define features the ORM might not support out of the box (for example: Postgres triggers).
Prisma supports custom migrations, so you can generate SQL with this tool and ship it alongside your Prisma migrations.

## Docs

[Read full docs](https://github.com/TheKnarf/dbschema/blob/main/docs/Readme.md)

## Install

```bash
cargo install dbschema
```

## Usage

Example: a small HCL file that defines a provider, an enum, a table, a trigger function, and a trigger.

```hcl
provider "postgres" {
  version = "16"
}

extension "pgcrypto" {}

enum "Status" {
  values = ["ACTIVE", "INACTIVE"]
}

table "users" {
  column "id" {
    type = "uuid"
    nullable = false
    default = "gen_random_uuid()"
  }
  column "email" {
    type = "text"
    nullable = false
  }
  column "status" {
    type = "Status"
    nullable = false
  }
  column "createdDate" {
    type = "timestamp"
    nullable = false
    default = "now()"
  }
  column "updatedDate" {
    type = "timestamp"
    nullable = true
  }

  primary_key {
    columns = ["id"]
  }
  index "users_email_key" {
    columns = ["email"]
    unique = true
  }
}

function "set_updated_at" {
  language = "plpgsql"
  returns  = "trigger"
  body = <<-SQL
  BEGIN
    NEW."updatedDate" := now();
    RETURN NEW;
  END;
  SQL
}

trigger "users_set_updated_at" {
  table = "users"
  timing = "BEFORE"
  events = ["UPDATE"]
  level  = "ROW"
  function = "set_updated_at"
}
```

If you omit the provider block dbschema will assume a default Postgres provider.

Create a migration (writes SQL to the given directory):

```bash
dbschema --input main.hcl create-migration --out-dir migrations --name init
```

## Development

- Ensure Rust toolchain is installed.
- Build:

```bash
cargo build
```

### Examples

- Validate all bundled examples: `just examples-validate`
- Create migrations for all examples: `just examples-create-migration`
- Run tests for a single example against Docker Postgres: `just example-test file=examples/table.hcl`
- Run tests for all examples against Docker Postgres: `just examples-test`

### Logging

This project uses [`env_logger`](https://docs.rs/env_logger) with `info` output enabled by default.
Set the `RUST_LOG` environment variable to control verbosity:

```bash
RUST_LOG=debug dbschema --input examples/table.hcl validate
```

Use `warn` or `error` to reduce output, e.g. `RUST_LOG=warn`.
