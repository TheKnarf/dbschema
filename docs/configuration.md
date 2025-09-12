# Configuration

dbschema can be configured using a `dbschema.toml` file in the root of your project. This file lets you define multiple generation targets, each with its own settings.

## Structure

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

## [settings] block

- input: Root HCL file (defaults to `main.hcl`).
- var_files: Variable files to load.
- env: Env vars to set before running a target.
- test_backend: Optional default backend for `test` (`postgres`).
- test_dsn: Optional default database connection string for tests when using Postgres.

## [[targets]] block

- name: Unique name for the target.
- description: Free-form description.
- backend: Generation backend (`postgres`, `prisma`, or `json`).
- input: Root HCL file for this target (overrides global `input`).
- output: Output file path (stdout if omitted).
- include: Resource kinds to include.
- exclude: Resource kinds to exclude.
- vars: Variables passed to HCL evaluation.
- var_files: Variable files to load for this target (in addition to global `var_files`).

