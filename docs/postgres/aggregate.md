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

## Examples

Define a simple aggregate that concatenates text values separated by commas.

```hcl
function "text_concat_sfunc" {
  language = "sql"
  returns  = "text"
  parameters = ["state text", "val text"]
  body = "SELECT CASE WHEN state IS NULL THEN val ELSE state || ',' || val END"
}

aggregate "text_concat" {
  sfunc  = "text_concat_sfunc"
  stype  = "text"
  initcond = "''"
}
```
