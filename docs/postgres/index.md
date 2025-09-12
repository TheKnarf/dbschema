# Index

Defines an index on an existing table.

```hcl
index "users_email_key" {
  table       = "users"
  columns     = ["email"]
  expressions = ["lower(name)"]
  orders      = ["ASC", "DESC"]
  operator_classes = ["text_pattern_ops"]
  where       = "email IS NOT NULL"
  unique      = true
}
```

## Attributes
- `name` (label): index name.
- `table` (string): table to index.
- `schema` (string, optional): schema of the table. Defaults to `public`.
- `columns` (array of strings): columns to include.
- `expressions` (array of strings, optional): expression items to include.
- `orders` (array of strings, optional): per-item sort order such as `ASC`, `DESC`, `NULLS FIRST`, or `NULLS LAST`.
- `operator_classes` (array of strings, optional): per-item operator class.
- `where` (string, optional): partial index predicate.
- `unique` (bool, optional): create a unique index.

## Examples

```hcl
index "users_email_key" { table = "users" columns = ["email"] unique = true }

index "posts_title_trgm" {
  table = "posts"
  method = "gin"
  expressions = ["title gin_trgm_ops"]
}

index "active_users_idx" {
  table = "users"
  columns = ["role"]
  where = "role <> 'suspended'"
}
```
