# Function

Creates a user-defined function.

```hcl
function "now_utc" {
  schema   = "public"
  language = "sql"
  returns  = "timestamptz"
  replace  = true
  body     = "SELECT now()"
}
```

## Attributes
- `name` (label): function name.
- `schema` (string, optional): schema for the function. Defaults to `public`.
- `language` (string): implementation language.
- `returns` (string): return type.
- `replace` (bool, optional): use `CREATE OR REPLACE`.
- `security_definer` (bool, optional): run as definer instead of invoker.
- `body` (string): function body.
- `comment` (string, optional): documentation comment.
