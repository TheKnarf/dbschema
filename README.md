# dbschema

A Rust CLI to define database schema's in a small HCL dialect, and generate idempotent SQL migrations.
It aims to support all PostgreSQL features (like extensions, functions, triggers, etc).


Designed to complement Prisma (or any tool) when you want to declaratively define features that the ORM might not support out of the box (ex. postgres triggers).
Prisma ORM support custom migrations, so you can use this tool to generate an SQL migration to add together with the other Prisma migrations.

## Features

- HCL blocks:
   - `variable`
   - `locals`
   - `schema`
   - `enum`
   - `table`
   - `view`
   - `materialized`
   - `function`
   - `trigger`
   - `extension`
   - `policy`
   - `role`
   - `grant`
   - `module`
   - `output`
   - `test`

- Variables via `--var key=value` and `--var-file`, with optional type and validation.
- Block repetition with `for_each` (arrays/objects) or numeric `count`.
- Dynamic blocks: replicate nested blocks with `dynamic "name" { for_each = ... content { ... } }`.
- Modules: `module "name" { source = "./path" ... }`.
- Full HCL expression support: numbers, booleans, arrays, objects, traversals (`var.*`, `local.*`), function calls, and `${...}` templates.
- Validate config, then generate SQL with safe `CREATE OR REPLACE FUNCTION` and idempotent guards for triggers/enums/materialized views.

## Install

- Ensure Rust toolchain is installed.
- Build:

```bash
cargo build --release
```

### Optional: PGlite in-memory backend

The PGlite runtime enables running tests without a real Postgres server. It is
gated behind the `pglite` feature and requires installing the WebAssembly
package:

```bash
just pglite-assets        # install @electric-sql/pglite into node_modules
cargo build --features pglite
```

Run tests with the PGlite backend:

```bash
cargo test --features pglite
```

Start an interactive PGlite shell:

```bash
./target/debug/dbschema pglite
```

Run HCL tests with PGlite either by passing the backend on the command line or
through `dbschema.toml`:

```bash
./target/debug/dbschema --input examples/main.hcl test --backend pglite

# dbschema.toml
[settings]
test_backend = "pglite"
```

## Usage

- Format HCL files: `./target/release/dbschema fmt [path]`
- Validate: `./target/release/dbschema --input examples/main.hcl validate`
- Create migration (Postgres SQL): `./target/release/dbschema --input examples/main.hcl create-migration --out-dir migrations --name triggers`
- Create Prisma models/enums only (no generator/datasource): `./target/release/dbschema --backend prisma --input examples/main.hcl create-migration --out-dir prisma --name schema`
- Variables: `--var schema=public` or `--var-file .env.hcl`
- Using config file: `dbschema --config` or `dbschema --config --target <target_name>`

## Logging

This project uses [`env_logger`](https://docs.rs/env_logger) with `info` output enabled by default.
Set the `RUST_LOG` environment variable to control verbosity:

```bash
RUST_LOG=debug dbschema --input examples/main.hcl validate
```

Use `warn` or `error` to reduce output, e.g. `RUST_LOG=warn`.

## Configuration File

dbschema can be configured using a `dbschema.toml` file in the root of your project. This file allows you to define multiple generation targets, each with its own settings.

### Structure

The configuration file consists of a global `[settings]` block and one or more `[[targets]]` blocks.

```toml
# Global settings (optional)
[settings]
input = "main.hcl"
var_files = ["vars.tfvars"]
env = { DATABASE_URL = "postgres://localhost:5432/mydb" }

# Target definitions
[[targets]]
name = "postgres_schema"
description = "Generate PostgreSQL schema for production"
backend = "postgres"
input = "main.hcl"
output = "schema.sql"
include = ["schemas", "tables", "views", "functions", "triggers", "extensions", "policies"]
exclude = []
vars = { environment = "production" }

[[targets]]
name = "prisma_client"
description = "Generate Prisma schema for client applications"
backend = "prisma"
input = "main.hcl"
output = "prisma/schema.prisma"
include = ["tables", "enums"]
exclude = ["functions", "triggers", "extensions", "policies", "views", "materialized"]
vars = { generate_client = "true" }
```

### `[settings]` block

- `input`: The root HCL file to use. Defaults to `main.hcl`.
- `var_files`: A list of variable files to load.
- `env`: A map of environment variables to set before running a target.
- `test_backend`: Optional default backend for the `test` command (`postgres` or `pglite`).
- `test_dsn`: Optional default database connection string for tests when using Postgres.

### `[[targets]]` block

- `name`: A unique name for the target.
- `description`: A description of the target.
- `backend`: The backend to use for generation (`postgres`, `prisma`, or `json`).
- `input`: The root HCL file for this target. Overrides the global `input` setting.
- `output`: The output file path. If not specified, the output is printed to stdout.
- `include`: A list of resource types to include.
- `exclude`: A list of resource types to exclude.
- `vars`: A map of variables to pass to the HCL evaluation context.
- `var_files`: A list of variable files to load for this target. These are loaded in addition to the global `var_files`.

## HCL Schema

```hcl
variable "<name>" {
  default = "value"
}

locals {
  some = "${var.name}-suffix"
}

function "<name>" {
  name     = "<new_name>"     # optional, overrides the block name
  schema   = "public"         # optional, default "public"
  language = "plpgsql"         # required (default plpgsql)
  returns  = "trigger"         # optional (default trigger)
  replace  = true               # optional (default true)
  security_definer = false      # optional
  body     = <<-SQL
    BEGIN
      NEW.updated_at = now();
      RETURN NEW;
    END;
  SQL
}

enum "<name>" {
  name   = "<new_name>"     # optional, overrides the block name
  schema = "public"    # optional, default "public"
  values = ["a", "b"]  # required
}

trigger "<name>" {
  name     = "<new_name>"     # optional, overrides the block name
  schema     = "public"        # optional, default "public"
  table      = "users"         # required
  timing     = "BEFORE"        # default BEFORE
  events     = ["UPDATE"]      # INSERT | UPDATE | DELETE (any combination)
  level      = "ROW"           # ROW | STATEMENT
  function   = "set_updated_at"# required (unqualified name)
  function_schema = "public"   # optional, defaults to trigger schema
  when       = null             # optional raw SQL condition
}

policy "<name>" {
  name    = "<new_name>"     # optional, overrides the block name
  schema  = "public"          # optional, default "public" (table schema)
  table   = "users"           # required
  as      = "permissive"       # optional: permissive|restrictive (default permissive)
  command = "select"           # optional: all|select|insert|update|delete (default all)
  roles   = ["app_user"]       # optional: omit for PUBLIC
  using   = "email is not null"# optional: USING (...) predicate
  check   = null               # optional: WITH CHECK (...) predicate
}

role "<name>" {
  name  = "<new_name>"  # optional, overrides the block name
  login = true           # optional, default false
}

grant "<name>" {
  role       = "app_user"           # grantee role (required)
  schema     = "public"             # optional, default "public"
  table      = "users"              # optional table target
  function   = null                  # optional function target
  privileges = ["SELECT"]          # required privileges
}

view "<name>" {
  name    = "<new_name>"     # optional, overrides the block name
  schema  = "public"     # optional, default "public"
  replace = true          # optional, default true (OR REPLACE)
  sql     = <<-SQL        # required SELECT ... body (no trailing semicolon needed)
    SELECT * FROM public.some_table
  SQL
}

materialized "<name>" {
  name      = "<new_name>"   # optional, overrides the block name
  schema    = "public"   # optional, default "public"
  with_data = true        # optional, default true (WITH [NO] DATA)
  sql       = <<-SQL      # required SELECT ... body
    SELECT ...
  SQL
}

table "<name>" {
  name          = "<new_name>"  # optional, overrides the block name
  schema        = "public"      # optional, default "public"
  if_not_exists = true          # optional, default true

  # Columns
  column "id" {
    type     = "serial"         # required (raw SQL type string)
    nullable = false            # optional (default true)
    default  = null             # optional raw SQL expression (e.g., "now()")
  }
  column "email" { type = "text" nullable = false }

  # Primary key (created inline only on CREATE TABLE)
  primary_key { columns = ["id"] }

  # Indexes (emitted as CREATE [UNIQUE] INDEX IF NOT EXISTS ... after table)
  unique "users_email_key" { columns = ["email"] }
  index  "users_created_idx" { columns = ["created_at"] }

  # Foreign keys (created inline only on CREATE TABLE)
  foreign_key {
    columns = ["org_id"]
    ref { schema = "public" table = "orgs" columns = ["id"] }
    on_delete = "CASCADE"      # optional
    on_update = "NO ACTION"    # optional
  }
}

extension "<name>" {
  name          = "<new_name>" # optional, overrides the block name
  # Creates `CREATE EXTENSION IF NOT EXISTS "<name>" [WITH SCHEMA "..." VERSION '...'];`
  if_not_exists = true     # optional, default true
  schema        = "public" # optional
  version       = "1.1"    # optional
}

module "<name>" {
  source = "./modules/timestamps"  # directory containing main.hcl
  schema = var.schema
  table  = "orders"
}
```

## Resource Filters

- Control which resources are included per run:
  - `--include tables --include functions` (repeatable)
  - `--exclude tables` (repeatable)
  - Resource kinds: `schemas, enums, tables, views, materialized, functions, triggers, extensions, policies, tests`
- Example split-output workflow:
  - Prisma models for tables: `dbschema --backend prisma --include tables --input examples/main.hcl create-migration --out-dir prisma --name schema`
  - SQL for everything else: `dbschema --backend postgres --exclude tables --input examples/main.hcl create-migration --out-dir migrations --name non_tables`
- Variables can be arrays/objects; use `for_each` on blocks and `each.value` inside.
- Tests can run against a real Postgres server or the in-memory PGlite backend; each test executes inside a transaction and is rolled back when using Postgres.
  - Assertion queries may return `bool`, any signed or unsigned integer (non-zero is treated as `true`), or text values `"t"`/`"true"` (case-insensitive).

## Expression Language

dbschema evaluates HCL expressions with support for strings, numbers, booleans, arrays, objects, function calls, traversals like `var.*` and `local.*`, and `${...}` string templates.

## Variables, for_each, dynamic blocks, and each.value

- Variables can be strings, numbers, booleans, arrays, or objects.
- Use `variable "name" { default = [...] }` or provide via `--var-file`.
- Optional `type` ("string", "number", "bool", "array", "object") and
  `validation` expression:

```hcl
variable "count" {
  type = "number"
  validation = var.count > 0
}
```

- dbschema enforces the declared type and runs the `validation` expression,
  returning friendly errors when they fail.
- Replicate blocks with `for_each` on the block (arrays or objects):
  - Arrays: `each.key` is the index (number), `each.value` is the element.
  - Objects: `each.key` is the object key (string), `each.value` is the value.
- Repeat blocks with a numeric `count` on the block; reference the index via `count.index`.
- Generate nested blocks with Terraform-style `dynamic` blocks:
  - Set `labels` to populate block labels when needed.
- Example:

```hcl
variable "tables" { default = ["users", "orders"] }

trigger "upd" {
  schema   = "public"
  for_each = var.tables       # will create two triggers
  table    = each.value       # "users" then "orders"
  timing   = "BEFORE"
  events   = ["UPDATE"]
  level    = "ROW"
  function = "set_updated_at"
}
```

Using `count` to repeat a block a fixed number of times:

```hcl
trigger "upd" {
  count    = 2
  name     = "set_updated_at_${count.index}"
  schema   = "public"
  table    = "users"
  timing   = "BEFORE"
  events   = ["UPDATE"]
  level    = "ROW"
  function = "set_updated_at"
}
```

Dynamic blocks allow repeated nested blocks. This example builds columns from a map:

```hcl
variable "cols" {
  default = {
    id   = { type = "serial", nullable = false }
    name = { type = "text",   nullable = true }
  }
}

table "users" {
  dynamic "column" {
    for_each = var.cols
    labels   = [each.key]
    content {
      type     = each.value.type
      nullable = each.value.nullable
    }
  }
}
```
