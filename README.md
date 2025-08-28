# dbschema (HCL → Postgres functions & triggers)

A tiny Rust CLI to define Postgres trigger functions and triggers in a small HCL dialect and generate idempotent SQL migrations. Designed to complement Prisma (or any tool) when you just want declarative triggers without a paid feature.

Status: early MVP. No network needed to read files; building requires Rust + crates.

## Features

- HCL blocks: `variable`, `locals`, `table`, `function`, `trigger`, `module`.
- Postgres `extension` blocks with options (schema, version, if_not_exists).
- Variables via `--var key=value` and `--var-file`.
- Modules (path-only): `module "name" { source = "./path" ... }`.
- Validate config, then generate SQL with safe `CREATE OR REPLACE FUNCTION` and idempotent `DO $$ IF NOT EXISTS CREATE TRIGGER $$`.

## Non-goals (for now)

- Full HCL expression language. Supported expressions: strings, numbers/bools (to-string), arrays of strings, traversals `var.*` and `local.*`, and string templates `${...}`. No arithmetic or conditionals yet.
- Module outputs/`module.*` references. Pass everything via inputs.
- Migration “down” scripts.

## Install

- Ensure Rust toolchain is installed.
- Build: `cargo build --release` (requires network to fetch crates on first build).

## Usage

- Validate: `./target/release/dbschema --input examples/main.hcl validate`
- Create migration (Postgres SQL): `./target/release/dbschema --input examples/main.hcl create-migration --out-dir migrations --name triggers`
- Create Prisma schema from tables: `./target/release/dbschema --backend prisma --input examples/main.hcl create-migration --out-dir prisma --name schema`
- Variables: `--var schema=public` or `--var-file .env.hcl`

## HCL Schema

function "<name>" {
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

trigger "<name>" {
  schema     = "public"        # optional, default "public"
  table      = "users"         # required
  timing     = "BEFORE"        # default BEFORE
  events     = ["UPDATE"]      # INSERT | UPDATE | DELETE (any combination)
  level      = "ROW"           # ROW | STATEMENT
  function   = "set_updated_at"# required (unqualified name)
  function_schema = "public"   # optional, defaults to trigger schema
  when       = null             # optional raw SQL condition
}

table "<name>" {
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
  # Creates `CREATE EXTENSION IF NOT EXISTS "<name>" [WITH SCHEMA "..." VERSION '...'];`
  if_not_exists = true     # optional, default true
  schema        = "public" # optional
  version       = "1.1"    # optional
}

variable "<name>" { default = "value" }

locals { some = "${var.name}-suffix" }

module "<name>" {
  source = "./modules/timestamps"  # directory containing main.hcl
  schema = var.schema
  table  = "orders"
}

## Examples

See `examples/main.hcl` and `examples/modules/timestamps/main.hcl`.

The root example now includes a `table` resource for `users`, a function, and a trigger.

## Notes

- Identifiers are always quoted. Strings embedded into `when` or function `body` are passed through as-is.
- Extension creation uses `CREATE EXTENSION [IF NOT EXISTS]` and is emitted before functions/triggers.
- Trigger creation is idempotent with a `DO $$` guard; function creation uses `CREATE OR REPLACE`.
- Table creation uses `CREATE TABLE IF NOT EXISTS` with inline primary keys and foreign keys. Indexes (including uniques) are emitted as `CREATE [UNIQUE] INDEX IF NOT EXISTS` after the table.
- Prisma backend: generates a Prisma schema with models for each `table`. It ignores functions/triggers/extensions.

## Resource Filters

- Control which resources are included per run:
  - `--include tables --include functions` (repeatable)
  - `--exclude tables` (repeatable)
- Example split-output workflow:
  - Prisma models for tables: `dbschema --backend prisma --include tables --input examples/main.hcl create-migration --out-dir prisma --name schema`
  - SQL for everything else: `dbschema --backend postgres --exclude tables --input examples/main.hcl create-migration --out-dir migrations --name non_tables`
- Variables can be arrays/objects; use `for_each` on blocks and `each.value` inside.
- Tests currently run against Postgres only; each test executes inside a transaction and is rolled back.

## Variables, for_each, and each.value

- Variables can be strings, numbers, booleans, arrays, or objects.
- Use `variable "name" { default = [...] }` or provide via `--var-file`.
- Replicate blocks with `for_each` on the block (arrays or objects):
  - Arrays: `each.key` is the index (number), `each.value` is the element.
  - Objects: `each.key` is the object key (string), `each.value` is the value.
- Example:

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
- Apply order: run your normal migrations first (e.g., Prisma), then this tool’s migration to ensure tables exist.

## Roadmap

- Module outputs and references (`module.foo.*`).
- Better expression support (concat, conditionals, maps).
- Optional `drop` generation for cleanup.
- Lints: existence of referenced tables/columns by inspecting a live DB (opt-in).
