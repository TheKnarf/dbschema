# Variables, Locals, and Repetition

## Locals

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

## Variables

Variables let you parameterize your schemas. Declare them with a `variable` block and reference using `var.<name>`.

```hcl
variable "schema" {
  type    = "string"
  default = "public"
  validation {
    condition     = var.schema != ""
    error_message = "schema must not be empty"
  }
}
```

### Typed variables

Variables may declare complex types to ensure the provided values match expectations:

```hcl
variable "ids" {
  type = "list(number)"
}

variable "labels" {
  type = "map(string)"
}
```

### `for_each` and `count`

Blocks can be repeated dynamically using `for_each` over a list or object, or a numeric `count`:

```hcl
trigger "upd" {
  for_each = var.tables
  name     = "set_updated_at_${each.value}"
  table    = each.value
  timing   = "BEFORE"
  events   = ["UPDATE"]
  level    = "ROW"
  function = "set_updated_at"
}

trigger "rep" {
  count = 2
  name  = "rep_${count.index}"
  ...
}
```

### Dynamic blocks

`dynamic` blocks replicate nested blocks. `each.key` and `each.value` are available inside the `content` section.

```hcl
variable "cols" {
  default = {
    id   = { type = "serial", nullable = false }
    name = { type = "text",   nullable = true }
  }
}

table "users" {
  dynamic "column" {
    for_each = var.cols
    labels   = [each.key]
    content {
      type     = each.value.type
      nullable = each.value.nullable
    }
  }
}
```

## Functions

Expressions may call a number of built‑in helpers. These are grouped
roughly by category:

* **String**: `upper`, `lower`, `length`, `substr`, `contains`,
  `startswith`, `endswith`, `trim`, `replace`
* **Numeric**: `min`, `max`, `abs`
* **Collections**: `concat`, `flatten`, `distinct`, `slice`, `sort`,
  `reverse`, `index`
* **Utility**: `coalesce`, `join`, `split`
* **Conversion**: `tostring`, `tonumber`, `tobool`, `tolist`, `tomap`
* **Crypto/Base64**: `md5`, `sha256`, `sha512`, `base64encode`,
  `base64decode`
* **Datetime**: `timestamp`, `formatdate`, `timeadd`, `timecmp`

These functions mirror those available in Terraform's expression
language and can be used anywhere an expression is accepted, including
within variable defaults and locals.
