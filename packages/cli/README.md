# @dbschema/cli

Node.js CLI wrapper for dbschema - a tool to define database schemas in HCL and generate SQL migrations.

This package provides a cross-platform CLI built on WebAssembly, making it easy to use dbschema without requiring the Rust toolchain.

## Installation

```bash
npm install -g @dbschema/cli
```

Or use with npx:

```bash
npx @dbschema/cli validate
```

## Usage

```bash
# Validate HCL schema
dbschema validate --input main.hcl

# Format HCL files
dbschema fmt

# Generate SQL migration
dbschema create-migration --backend postgres --out-dir migrations --name init

# See all commands
dbschema --help
```

## Available Commands

### `validate`

Validate HCL configuration and print a summary.

```bash
dbschema validate [options]

Options:
  --input <file>              Root HCL file (default: "main.hcl")
  --strict                    Enable strict mode
  --include <resources...>    Include only these resources
  --exclude <resources...>    Exclude these resources
```

### `fmt`

Format HCL files in place.

```bash
dbschema fmt [paths...]

Arguments:
  paths    Files or directories to format (default: ["."])
```

### `create-migration`

Create a SQL migration file from HCL.

```bash
dbschema create-migration [options]

Options:
  --input <file>              Root HCL file (default: "main.hcl")
  --backend <backend>         Backend: postgres, prisma, or json (default: "postgres")
  --out-dir <dir>             Output directory for migration files
  --name <name>               Migration name
  --strict                    Enable strict mode
  --include <resources...>    Include only these resources
  --exclude <resources...>    Exclude these resources
```

## Limitations

Some features require the native Rust CLI (install with `cargo install dbschema`):

- **`test` command**: Requires PostgreSQL connection (not available in WebAssembly)
- **`lint` command**: The `sql-syntax` lint check requires native pg_query library

For these features, use the native CLI:

```bash
cargo install dbschema
dbschema test --dsn postgresql://localhost/mydb
dbschema lint
```

## Example

```bash
# Create a schema file
cat > main.hcl <<EOF
table "users" {
  column "id" {
    type = "serial"
    nullable = false
  }
  column "email" {
    type = "text"
    nullable = false
  }
  primary_key { columns = ["id"] }
  index "users_email_key" { columns = ["email"] unique = true }
}
EOF

# Validate it
dbschema validate

# Format it
dbschema fmt

# Generate SQL
dbschema create-migration --out-dir migrations --name users
```

## Related Packages

- `dbschema` - Core WASM library for programmatic use
- Native CLI - Full-featured Rust CLI: `cargo install dbschema`

## Documentation

For full documentation, see the [main dbschema repository](https://github.com/your-org/dbschema).

## License

MIT