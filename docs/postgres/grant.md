# Grant

Grants privileges to a role on database objects.

```hcl
grant "app_user_db" {
  role       = "app_user"
  privileges = ["ALL"]
  database   = "appdb"
}

grant "app_user_seq" {
  role       = "app_user"
  privileges = ["USAGE"]
  schema     = "public"
  sequence   = "user_id_seq"
}
```

## Attributes
- `name` (label): identifier for the grant.
- `role` (string): role receiving the privileges.
- `privileges` (array of strings): privileges such as `SELECT`, `INSERT`, etc.
- `schema` (string, optional): schema containing the object.
- `table` (string, optional): table name.
- `function` (string, optional): function name.
- `database` (string, optional): database name.
- `sequence` (string, optional): sequence name.
- `privileges = ["ALL"]` grants all privileges.

## Examples

```hcl
grant "app_user_table" {
  role = "app_user"
  schema = "public"
  table = "docs"
  privileges = ["SELECT", "INSERT", "UPDATE"]
}

grant "app_user_functions" {
  role = "app_user"
  schema = "public"
  function = "set_updated_at"
  privileges = ["EXECUTE"]
}
```
