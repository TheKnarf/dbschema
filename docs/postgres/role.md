# Role

Creates a database role.

```hcl
role "app_user" {
  login      = true
  createdb   = true
  password   = "secret"
  in_role    = ["base_user"]
}
```

## Attributes
- `name` (label): role name.
- `login` (bool, optional): allow login. Defaults to `false`.
- `superuser` (bool, optional): allow superuser privileges. Defaults to `false`.
- `createdb` (bool, optional): allow creating databases. Defaults to `false`.
- `createrole` (bool, optional): allow creating roles. Defaults to `false`.
- `replication` (bool, optional): allow replication. Defaults to `false`.
- `password` (string, optional): role password.
- `in_role` (array of strings, optional): roles this role will be added to.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
role "app_user" { login = true }
role "app_admin" { login = true in_role = ["app_user"] }
```
