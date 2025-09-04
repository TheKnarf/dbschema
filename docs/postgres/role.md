# Role

Creates a database role.

```hcl
role "app_user" {
  login = false
}
```

## Attributes
- `name` (label): role name.
- `login` (bool, optional): allow login. Defaults to `false`.
- `comment` (string, optional): documentation comment.
