# Publication

Defines a replication publication.

```hcl
publication "pub" {
  tables = [
    { schema = "public", table = "t" }
  ]
  publish = ["insert", "update"]
}
```

## Attributes
- `name` (label): publication name.
- `all_tables` (bool): when `true`, publish all tables.
- `tables` (list of objects, optional): tables to publish. Each object has `schema` (optional) and `table`.
- `publish` (list of string, optional): operations to publish.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
publication "pub_all" { all_tables = true }

publication "pub_some" {
  tables = [
    { schema = "public", table = "users" },
    { table = "posts" }
  ]
  publish = ["insert", "update"]
}
```
