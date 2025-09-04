# Enum

Creates a PostgreSQL enum type with a fixed set of values.

```hcl
enum "status" {
  schema = "public"
  values = ["active", "disabled"]
}
```

## Attributes
- `name` (label): enum type name.
- `schema` (string, optional): schema for the type. Defaults to `public`.
- `values` (array of strings): ordered list of allowed values.
- `comment` (string, optional): documentation comment.
