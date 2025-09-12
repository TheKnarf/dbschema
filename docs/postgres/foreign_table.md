# Foreign Table

Defines a foreign table linked to a foreign server.

```hcl
foreign_table "remote" {
  schema = "public"
  server = "my_srv"
  column "id" { type = "int", nullable = false }
  options = ["schema_name 'public'", "table_name 'remote_table'"]
}
```

## Attributes
- `name` (label): table name.
- `schema` (string, optional): schema for the table. Defaults to `public`.
- `server` (string): foreign server name.
- `column` blocks: column definitions (`type`, `nullable`, optional `default`, `db_type`, `lint_ignore`, `comment`).
- `options` (array of strings, optional): additional `OPTIONS` entries.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
foreign_table "rt" {
  server = "csv_srv"
  column "id" { type = "int", nullable = false }
  options = ["filename '/tmp/data.csv'"]
}
```
