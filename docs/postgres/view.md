# View

Creates a SQL view.

```hcl
view "active_users" {
  schema  = "public"
  replace = true
  sql = <<-SQL
    SELECT id, email FROM public.users
  SQL
}
```

## Attributes
- `name` (label): view name.
- `schema` (string, optional): schema for the view. Defaults to `public`.
- `replace` (bool, optional): use `CREATE OR REPLACE VIEW`.
- `sql` (string): SELECT statement defining the view.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
view "active_users" {
  sql = <<-SQL
    SELECT id, email
    FROM users
    WHERE active = true
  SQL
}
```
