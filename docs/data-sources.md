# Data Sources

Data sources let you read external inputs and expose them to other resources in your HCL. They are declared with
`data` blocks that contain a type and a logical name. The loaded value is available through the global `data`
namespace, e.g. `data.prisma_schema.app`.

```hcl
data "prisma_schema" "app" {
  file = "prisma/schema.prisma"
}

locals {
  user_model = data.prisma_schema.app.models.User
}
```

## Prisma schema data source

`prisma_schema` parses a Prisma schema file and returns its contents as HCL objects that mirror Prisma's structure. The
schema is resolved relative to the current module directory. The returned value contains the following top-level keys:

- `models` — map keyed by model name. Each entry exposes `name`, `fields`, and `attributes`.
- `enums` — map keyed by enum name. Each entry exposes `name`, `values`, and `attributes`.

You can pull individual model fields or enum definitions into tables, domains, or other resources. Example: mirror a
subset of a Prisma model into a Postgres table.

```hcl
data "prisma_schema" "app" {
  file = "schema.prisma"
}

provider "postgres" {}

schema "app" {}

table "app" "users" {
  column "id" {
    type     = "uuid"
    nullable = false
    default  = "gen_random_uuid()"
  }

  dynamic "column" {
    for_each = data.prisma_schema.app.models.User.fields

    content {
      name     = column.value.name
      type     = column.value.type.name
      nullable = column.value.type.optional
    }
  }
}
```

If your Prisma schema already defines enums, you can reuse them directly:

```hcl
enum "Status" {
  values = data.prisma_schema.app.enums.Status.values.*.name
}
```

The schema loader preserves the raw attribute strings, so you can inspect defaults, uniqueness, or IDs for more complex
logic. Unsupported data source types result in a validation error during loading.
