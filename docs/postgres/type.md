# Composite Type

Defines a custom composite type with named fields.

```hcl
type "address" {
  field "street" { type = "text" }
  field "zip"    { type = "int" }
}
```

## Attributes
- `name` (label): type name.
- `schema` (string, optional): schema for the type. Defaults to `public`.
- `field` blocks: each adds a field with a `type`.
- `comment` (string, optional): documentation comment.
