# Schema

Defines a database schema. Optional attributes control existence checks and ownership.

```hcl
schema "analytics" {
  if_not_exists = true
  authorization = "app_user"
}
```

## Attributes
- `name` (label): schema name.
- `if_not_exists` (bool): emit `CREATE SCHEMA IF NOT EXISTS` when true. Defaults to `false`.
- `authorization` (string, optional): owner of the schema.
- `comment` (string, optional): documentation comment.
