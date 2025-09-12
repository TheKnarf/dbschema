# Linting

`dbschema lint` runs built-in checks against your schema. The default checks are:

- naming-convention: table and column names must be snake_case.
- missing-index: tables should define at least one index or primary key.
- forbid-serial: disallow use of serial/bigserial column types.
- primary-key-not-null: columns in a primary key must be NOT NULL.
- destructive-change: foreign keys using ON DELETE/ON UPDATE CASCADE.
- unused-index: indexes that duplicate a table's primary key.
- long-identifier: table, column, or index names longer than 63 characters.
- missing-foreign-key-index: foreign key columns should be indexed.
- column-type-mismatch: foreign key column types must match referenced columns.

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

Additional suppressions:

```hcl
table "orders" {
  lint_ignore = ["missing-foreign-key-index"]

  column "user_id" {
    type = "text"
    lint_ignore = ["column-type-mismatch"]
  }

  foreign_key {
    columns = ["user_id"]
    ref_table = "users"
    ref_columns = ["id"]
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
