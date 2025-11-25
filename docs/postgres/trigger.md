# Trigger

Attaches a function to table events.

```hcl
trigger "users_updated_at" {
  schema   = "public"
  table    = "users"
  timing   = "BEFORE"
  events   = ["UPDATE"]
  level    = "ROW"
  function = "set_updated_at"
}
```

## Attributes
- `name` (label): trigger name.
- `schema` (string, optional): schema for the trigger. Defaults to `public`.
- `table` (string): table the trigger operates on.
- `timing` (string): `BEFORE` or `AFTER`.
- `events` (array of strings): `INSERT`, `UPDATE`, `DELETE`.
- `level` (string): `ROW` or `STATEMENT`.
- `function` (string): function name to execute.
- `function_schema` (string, optional): schema of the function.
- `when` (string, optional): optional WHEN condition.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
table "users" {
  column "id" {
    type = "uuid"
    nullable = false
    default = "gen_random_uuid()"
  }
  column "email" {
    type = "text"
    nullable = false
  }
  column "updatedDate" {
    type = "timestamp"
    nullable = true
  }
  primary_key {
    columns = ["id"]
  }
}

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

trigger "users_set_updated_at" {
  table = "users"
  timing = "BEFORE"
  events = ["UPDATE"]
  level  = "ROW"
  function = "set_updated_at"
}
```
