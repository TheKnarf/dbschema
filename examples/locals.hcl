locals {
  schema = "public"
}

table "users" {
  schema = local.schema
  column "id" { type = "serial" nullable = false }
}

test "local_schema" {
  assert = "SELECT to_regclass('public.users') IS NOT NULL"
}
