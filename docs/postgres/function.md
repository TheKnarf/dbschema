# Function

Creates a user-defined function.

```hcl
function "now_utc" {
  schema      = "public"
  language    = "sql"
  parameters  = ["arg1 int"]
  returns     = "timestamptz"
  replace     = true
  volatility  = "immutable"
  strict      = true
  security    = "definer"
  cost        = 100
  body        = "SELECT now()"
}
```

## Attributes
- `name` (label): function name.
- `schema` (string, optional): schema for the function. Defaults to `public`.
- `language` (string): implementation language.
- `parameters` (list of strings, optional): function parameters.
- `returns` (string): return type.
- `replace` (bool, optional): use `CREATE OR REPLACE`.
- `volatility` (string, optional): `immutable`, `stable`, or `volatile`.
- `strict` (bool, optional): use `STRICT` (defaults to `CALLED ON NULL INPUT`).
- `security` (string, optional): `definer` or `invoker`.
- `cost` (number, optional): estimated execution cost.
- `body` (string): function body.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
function "set_updated_at" {
  language = "plpgsql"
  returns  = "trigger"
  body = <<-SQL
  BEGIN
    NEW."updatedDate" := now();
    RETURN NEW;
  END;
  SQL
}

function "add" {
  language = "sql"
  returns  = "int"
  parameters = ["a int", "b int"]
  body = "SELECT a + b"
}
```
