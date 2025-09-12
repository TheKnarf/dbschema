# Foreign Server

Creates a foreign server for a data wrapper.

```hcl
foreign_server "my_srv" {
  wrapper = "my_fdw"
  type    = "postgres"
  options = ["host 'remote'"]
}
```

## Attributes
- `name` (label): server name.
- `wrapper` (string): associated foreign data wrapper.
- `type` (string, optional): server type.
- `version` (string, optional): server version.
- `options` (array of strings, optional): additional `OPTIONS` entries.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
foreign_server "csv_srv" {
  wrapper = "csv_fdw"
  options = ["filename '/tmp/data.csv'"]
}
```
