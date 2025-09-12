# Statistics

Defines an extended statistics object for a table.

```hcl
statistics "orders_stats" {
  schema  = "public"
  table   = "orders"
  columns = ["region", "product"]
  kinds   = ["ndistinct", "dependencies"]
  comment = "Multi-column statistics for orders"
}
```

## Attributes
- `name` (label): statistics name.
- `schema` (string, optional): schema of the table. Defaults to `public`.
- `table` (string): table the statistics are based on.
- `columns` (array of strings): columns to include in the statistics.
- `kinds` (array of strings, optional): statistics kinds such as `ndistinct`, `dependencies`, or `mcv`.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
statistics "orders_stats" {
  table   = "orders"
  columns = ["region", "product"]
}
```
