# Operator

Defines a user-defined operator.

```hcl
operator "text_prefix" {
  procedure = "text_prefix_compare"
  left  = "text"
  right = "text"
}
```

## Attributes
- `name` (label): operator name.
- `schema` (string, optional): schema for the operator. Defaults to `public`.
- `left` (string, optional): left operand type.
- `right` (string, optional): right operand type.
- `procedure` (string): function implementing the operator.
- `commutator` (string, optional): commutator operator name.
- `negator` (string, optional): negator operator name.
- `restrict` (string, optional): restrict function name.
- `join` (string, optional): join function name.
- `comment` (string, optional): comment for the operator.

## Examples

```hcl
operator "int_mul" {
  procedure = "int4mul"
  left = "int4"
  right = "int4"
}

operator "factorial" {
  procedure = "factorial"
  right = "int"
}
```
