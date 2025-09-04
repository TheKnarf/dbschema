# Table

Creates a table with columns and constraints.

```hcl
table "users" {
  schema       = "public"
  if_not_exists = true

  column "id" {
    type     = "serial"
    nullable = false
  }

  column "email" {
    type     = "text"
    nullable = false
  }

  primary_key { columns = ["id"] }
  check "email_not_empty" { expression = "email <> ''" }
}
```

## Attributes
- `name` (label): table name.
- `schema` (string, optional): schema for the table. Defaults to `public`.
- `if_not_exists` (bool, optional): emit `IF NOT EXISTS`.
- `column` blocks: define columns with `type`, `nullable`, optional `default`, `db_type`, `lint_ignore`, `comment`.
- `primary_key` block: list of column names and optional constraint name.
- `check` blocks: named check constraints with an `expression`.
- `index` blocks: inline index definitions (`columns`, `unique`).
- `foreign_key` blocks: reference other tables with `columns`, `ref_schema`, `ref_table`, `ref_columns`, `on_delete`, `on_update`.
- `back_reference` blocks: create foreign keys on another table.
- `lint_ignore` (array of strings, optional): suppress lint rules.
- `comment` (string, optional): documentation comment.
