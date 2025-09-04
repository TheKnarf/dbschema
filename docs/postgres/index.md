# Index

Defines an index on an existing table.

```hcl
index "users_email_key" {
  table   = "users"
  columns = ["email"]
  unique  = true
}
```

## Attributes
- `name` (label): index name.
- `table` (string): table to index.
- `schema` (string, optional): schema of the table. Defaults to `public`.
- `columns` (array of strings): columns to include.
- `unique` (bool, optional): create a unique index.
