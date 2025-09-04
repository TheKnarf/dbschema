# Materialized View

Defines a materialized view that stores query results.

```hcl
materialized "user_counts" {
  schema    = "public"
  with_data = true
  sql = <<-SQL
    SELECT 1 as id
  SQL
}
```

## Attributes
- `name` (label): view name.
- `schema` (string, optional): schema for the view. Defaults to `public`.
- `with_data` (bool, optional): include `WITH DATA` (default) or `WITH NO DATA`.
- `sql` (string): SELECT statement defining the view.
- `comment` (string, optional): documentation comment.
