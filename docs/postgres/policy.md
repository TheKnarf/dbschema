# Policy

Defines a row-level security policy.

```hcl
policy "user_select" {
  schema  = "public"
  table   = "users"
  command = "select"
  roles   = ["postgres"]
  using   = "true"
}
```

## Attributes
- `name` (label): policy name.
- `schema` (string, optional): schema of the table. Defaults to `public`.
- `table` (string): table the policy applies to.
- `command` (string): `ALL`, `SELECT`, `INSERT`, `UPDATE`, or `DELETE`.
- `as` (string, optional): `PERMISSIVE` or `RESTRICTIVE`.
- `roles` (array of strings): roles the policy applies to. Empty means `PUBLIC`.
- `using` (string, optional): expression for row visibility.
- `check` (string, optional): expression for permitted values on write.
- `comment` (string, optional): documentation comment.
