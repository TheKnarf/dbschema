# Domain

Defines a custom domain built on top of a base type with optional constraints.

```hcl
domain "email" {
  type = "text"
  check = "VALUE ~* '^[^@]+@[^@]+$'"
}
```

## Attributes
- `name` (label): domain name.
- `schema` (string, optional): schema for the domain. Defaults to `public`.
- `type` (string): underlying data type.
- `not_null` (bool, optional): add `NOT NULL` constraint.
- `default` (string, optional): default value expression.
- `constraint` (string, optional): name of a constraint.
- `check` (string, optional): `CHECK` expression using `VALUE`.
- `comment` (string, optional): documentation comment.
