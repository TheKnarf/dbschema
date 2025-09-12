# create-migration

Generate an artifact (SQL for Postgres, Prisma schema, or JSON) from your HCL.

## Usage

```bash
dbschema --input main.hcl --backend postgres create-migration \
  --out-dir migrations \
  --name init
```

- Writes a file like `migrations/<timestamp>_init.sql` (Postgres) or `migrations/<timestamp>_init.prisma` (Prisma).
- If `--out-dir` is omitted, the artifact is printed to stdout.

## Options

Subcommand options:
- `--out-dir <dir>`: Output directory. If provided, creates a timestamped file.
- `--name <string>`: Optional name used in the output filename (defaults to `triggers`).

Global options that affect generation:
- `--input <path>`: Root HCL file (default: `main.hcl`).
- `--backend <postgres|prisma|json>`: Backend to generate for (default: `postgres`).
- `--include <kind>` / `--exclude <kind>`: Filter resource kinds.
- `--var key=value` / `--var-file <path>`: Provide variables.
- `--strict`: Error if an enum type referenced in tables isn’t defined.

Common resource kinds for `--include/--exclude`:
- `schemas, sequences, enums, tables, views, materialized, functions, triggers, event_triggers, extensions, policies, tests`

## Examples

Generate Postgres SQL (tables + functions/triggers), write to a file:
```bash
dbschema --input main.hcl --backend postgres \
  --include tables --include functions --include triggers \
  create-migration --out-dir migrations --name schema
```

Generate Prisma schema (models/enums only), to stdout:
```bash
dbschema --input main.hcl --backend prisma \
  --include tables --include enums \
  create-migration
```

Generate JSON IR for inspection:
```bash
dbschema --input main.hcl --backend json create-migration --name plan --out-dir artifacts
```
