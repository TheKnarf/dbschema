# Sequence

Defines an auto-incrementing sequence.

```hcl
sequence "user_id_seq" {
  schema    = "public"
  as        = "bigint"
  increment = 1
  min_value = 1
  start     = 1
  cache     = 1
  cycle     = false
  owned_by  = "users.id"
}
```

## Attributes
- `name` (label): sequence name.
- `schema` (string, optional): schema for the sequence. Defaults to `public`.
- `if_not_exists` (bool, optional): emit `IF NOT EXISTS`.
- `as` (string, optional): data type of the sequence.
- `increment`, `min_value`, `max_value`, `start`, `cache` (numbers, optional): control sequence behavior.
- `cycle` (bool, optional): wrap around when reaching limits.
- `owned_by` (string, optional): table column this sequence is owned by.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
sequence "order_id_seq" {
  as = "bigint"
  start = 1000
  increment = 1
}

table "orders" {
  column "id" { type = "bigint", nullable = false, default = "nextval('order_id_seq')" }
  primary_key { columns = ["id"] }
}
```
