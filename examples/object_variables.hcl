variable "table_name" {
  type    = "string"
  default = "object_variable_example"
}

variable "columns" {
  type = "list(object({ name = string, type = string, nullable = optional(bool) }))"
  default = [
    { name = "label",   type = "text",  nullable = true },
    { name = "payload", type = "jsonb", nullable = false },
  ]
}

table "object_variable_example" {
  schema = "public"
  table_name = var.table_name

  column "id" {
    type     = "text"
    nullable = false
  }

  dynamic "column" {
    for_each = var.columns
    labels   = [each.value.name]
    content {
      type     = each.value.type
      nullable = each.value.nullable
    }
  }

  primary_key { columns = ["id"] }
}

test "object_variable_columns" {
  assert = [
    "SELECT COUNT(*) = 3 FROM information_schema.columns WHERE table_schema = 'public' AND table_name = '${var.table_name}'",
    "SELECT array_agg(column_name::text ORDER BY ordinal_position) = ARRAY['id', 'label', 'payload'] FROM information_schema.columns WHERE table_schema = 'public' AND table_name = '${var.table_name}'"
  ]
}
