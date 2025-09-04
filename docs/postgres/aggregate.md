# Aggregate

Creates a user-defined aggregate function.

```hcl
aggregate "sum_int" {
  schema  = "public"
  inputs  = ["int"]
  sfunc   = "int_sum"
  stype   = "int"
  finalfunc = "int4_sum_final"
  initcond  = "0"
  parallel  = "safe"
}
```

## Attributes
- `name` (label): aggregate name.
- `schema` (string, optional): schema for the aggregate. Defaults to `public`.
- `inputs` (list of strings, optional): argument data types.
- `sfunc` (string): state transition function.
- `stype` (string): state data type.
- `finalfunc` (string, optional): final function.
- `initcond` (string, optional): initial state value.
- `parallel` (string, optional): `safe`, `restricted`, or `unsafe`.
- `comment` (string, optional): documentation comment.
