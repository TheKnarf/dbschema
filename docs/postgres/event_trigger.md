# Event Trigger

Fires a function in response to database-wide events.

```hcl
event_trigger "log_ddl" {
  event   = "ddl_command_start"
  tags    = ["CREATE TABLE"]
  function = "ddl_logger"
}
```

## Attributes
- `name` (label): trigger name.
- `event` (string): event name such as `ddl_command_start`.
- `tags` (array of strings): optional filter on `TAG IN (...)`.
- `function` (string): function to execute.
- `function_schema` (string, optional): schema of the function.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
function "audit_ddl" {
  language = "plpgsql"
  returns  = "event_trigger"
  body = <<-SQL
  BEGIN
    RAISE NOTICE 'DDL: %', tg_tag;
  END;
  SQL
}

event_trigger "audit_ddl_start" {
  event = "ddl_command_start"
  function = "audit_ddl"
}
```
