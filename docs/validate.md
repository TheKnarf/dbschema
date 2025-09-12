# validate

Validate your HCL and print a summary of discovered resources.

## Usage

```bash
dbschema --input main.hcl validate
```

Typical output prints counts of schemas, enums, tables, views, functions, triggers, etc.

## Options

Global options that affect validation:
- `--input <path>`: Root HCL file (default: `main.hcl`).
- `--backend <postgres|prisma|json>`: Only used to interpret types for certain checks (default: `postgres`).
- `--include <kind>` / `--exclude <kind>`: Filter resource kinds before validation.
- `--var key=value` / `--var-file <path>`: Provide variables for evaluation.
- `--strict`: Error if an enum type referenced in tables isn’t defined.

## Examples

Validate everything with a variable file:
```bash
dbschema --input main.hcl --var-file env.hcl validate
```

Validate tables and functions only:
```bash
dbschema --input main.hcl --include tables --include functions validate
```
