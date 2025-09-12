# Procedure

Defines a stored procedure.

```hcl
procedure "log_action" {
  schema     = "public"
  language   = "plpgsql"
  parameters = ["p_id int"]
  replace    = true
  security   = "definer"
  body = <<-SQL
  BEGIN
    INSERT INTO log_table(id) VALUES (p_id);
  END;
  SQL
}
```

## Attributes
- `name` (label): procedure name.
- `schema` (string, optional): schema for the procedure. Defaults to `public`.
- `language` (string): implementation language.
- `parameters` (list of strings, optional): procedure parameters.
- `replace` (bool, optional): use `CREATE OR REPLACE`.
- `security` (string, optional): `definer` or `invoker`.
- `body` (string): procedure body.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
procedure "do_nothing" {
  language = "plpgsql"
  body = "BEGIN NULL; END;"
}
```

