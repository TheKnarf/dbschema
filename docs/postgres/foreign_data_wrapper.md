# Foreign Data Wrapper

Defines a foreign data wrapper.

```hcl
foreign_data_wrapper "my_fdw" {
  handler  = "fdw_handler"
  validator = "fdw_validator"
  options = ["host 'remote'"]
}
```

## Attributes
- `name` (label): wrapper name.
- `handler` (string, optional): handler function.
- `validator` (string, optional): validator function.
- `options` (array of strings, optional): additional `OPTIONS` entries.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
foreign_data_wrapper "csv_fdw" {
  handler = "csv_fdw_handler"
  options = ["delimiter ','"]
}
```
