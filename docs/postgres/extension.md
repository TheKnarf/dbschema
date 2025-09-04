# Extension

Installs a PostgreSQL extension.

```hcl
extension "pgcrypto" {
  if_not_exists = true
}
```

## Attributes
- `name` (label): extension name.
- `if_not_exists` (bool, optional): emit `IF NOT EXISTS` (defaults to true).
- `schema` (string, optional): target schema for extension objects.
- `version` (string, optional): specific version to install.
- `comment` (string, optional): documentation comment.
