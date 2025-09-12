# Collation

Defines a text collation used for locale-aware sorting.

```hcl
collation "en_us" {
  schema = "public"
  locale = "en_US"
  provider = "icu"
}
```

## Attributes
- `name` (label): collation name.
- `schema` (string, optional): schema for the collation. Defaults to `public`.
- `if_not_exists` (bool, optional): emit `IF NOT EXISTS`.
- `from` (string, optional): copy from an existing collation.
- `locale` (string, optional): sets `LOCALE`.
- `lc_collate` (string, optional): sets `LC_COLLATE`.
- `lc_ctype` (string, optional): sets `LC_CTYPE`.
- `provider` (string, optional): provider such as `icu` or `libc`.
- `deterministic` (bool, optional): emit `DETERMINISTIC = true|false`.
- `version` (string, optional): sets `VERSION`.
- `comment` (string, optional): documentation comment.
