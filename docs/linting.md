# Linting

`dbschema lint` runs built-in checks against your schema. The default checks are:

- naming-convention: table and column names must be snake_case.
- missing-index: tables should define at least one index or primary key.
- forbid-serial: disallow use of serial/bigserial column types.
- primary-key-not-null: columns in a primary key must be NOT NULL.
- destructive-change: foreign keys using ON DELETE/ON UPDATE CASCADE.
- unused-index: indexes that duplicate a table's primary key.
- long-identifier: table, column, or index names longer than 63 characters.

Suppress a rule for a specific table or column with `lint_ignore`:

```hcl
table "users" {
  lint_ignore = ["missing-index"]

  column "ID" {
    type = "int"
    lint_ignore = ["naming-convention"]
  }
}
```

Configure rule severity globally in `dbschema.toml`:

```toml
[settings.lint.severity]
missing-index = "warn"
forbid-serial = "error"
```

Setting a rule's severity to `allow` suppresses it entirely.
Severity can also be overridden on the command line using `--allow`, `--warn`, or `--error` flags:

```sh
dbschema lint --warn missing-index --allow long-identifier
```
