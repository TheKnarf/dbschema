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

  partition_by {
    strategy = "RANGE"
    columns  = ["id"]
  }
  partition "users_1" { values = "FROM (1) TO (100)" }

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
- `partition_by` block: define partitioning `strategy` (`RANGE`, `LIST`, `HASH`) and `columns`.
- `partition` blocks: create child partitions with a name and `values` bounds string.
- `back_reference` blocks: create foreign keys on another table.
- `lint_ignore` (array of strings, optional): suppress lint rules.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
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
  column "role" {
    type = "text"
    nullable = false
    default = "user"
  }
  primary_key {
    columns = ["id"]
  }
  index "users_email_key" {
    columns = ["email"]
    unique = true
  }
}

table "posts" {
  column "id" {
    type = "uuid"
    nullable = false
    default = "gen_random_uuid()"
  }
  column "authorId" {
    type = "uuid"
    nullable = false
  }
  column "title" {
    type = "text"
    nullable = false
  }
  column "body" {
    type = "text"
    nullable = true
  }
  primary_key { columns = ["id"] }
  foreign_key {
    columns = ["authorId"]
    ref {
      table = "users"
      columns = ["id"]
    }
    on_delete = "CASCADE"
  }
}
```
