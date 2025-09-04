# Locals

`locals` define named expressions that can be referenced throughout the configuration using `local.<name>`.

```hcl
locals {
  schema = "public"
  table  = "users"
}

table "users" {
  schema = local.schema
  column "id" { type = "serial", nullable = false }
}
```

Locals are evaluated once and are useful for computed values or to avoid repeating literals.
