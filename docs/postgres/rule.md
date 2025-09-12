# Rule

Defines a query rewrite rule for a table.

```hcl
rule "notify_users" {
  table   = "users"
  event   = "UPDATE"
  command = "NOTIFY users_changed"
}
```

## Attributes
- `name` (label): rule name.
- `table` (string): target table.
- `schema` (string, optional): schema of the table. Defaults to `public`.
- `event` (string): triggering event.
- `where` (string, optional): condition expression.
- `instead` (bool, optional): use `INSTEAD` instead of `ALSO`.
- `command` (string): command to execute when the rule fires.
- `comment` (string, optional): comment for the rule.

## Examples

```hcl
rule "no_delete_users" {
  table   = "users"
  event   = "DELETE"
  instead = true
  command = "RAISE EXCEPTION 'cannot delete'"
}
```
