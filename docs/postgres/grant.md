# Grant

Grants privileges to a role on database objects.

```hcl
grant "app_user_tables" {
  role       = "app_user"
  privileges = ["SELECT"]
  schema     = "public"
  table      = "users"
}
```

## Attributes
- `name` (label): identifier for the grant.
- `role` (string): role receiving the privileges.
- `privileges` (array of strings): privileges such as `SELECT`, `INSERT`, etc.
- `schema` (string, optional): schema containing the object.
- `table` (string, optional): table name.
- `function` (string, optional): function name.
