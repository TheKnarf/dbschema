variable "schema" { default = "public" }

table "users" {
  schema = var.schema
  column "id" {
    type = "serial"
    nullable = false
  }
}

test "variable_schema" {
  assert = "SELECT to_regclass('public.users') IS NOT NULL"
}
