# Tablespace

Defines a PostgreSQL tablespace which stores data files in a specific location.

```hcl
tablespace "fastspace" {
  location = "/mnt/ssd1"
  owner    = "app_user"
}
```

## Attributes
- `name` (label): tablespace name.
- `location` (string): directory for the tablespace.
- `owner` (string, optional): role that owns the tablespace.
- `options` (list of strings, optional): additional `WITH` options.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
tablespace "fastspace" {
  location = "/mnt/ssd1"
  owner    = "app_user"
  options  = ["seq_page_cost=1.0"]
}
```
