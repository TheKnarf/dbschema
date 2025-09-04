table "users" {
  schema = "public"
  if_not_exists = true

  column "id" {
    type = "serial"
    nullable = false
  }
  column "email" {
    type = "text"
    nullable = false
  }

  primary_key { columns = ["id"] }
  check "email_not_empty" {
    expression = "email <> ''"
  }
}

index "users_email_key" {
  table   = "users"
  columns = ["email"]
  unique  = true
}

test "table" {
  assert = "SELECT to_regclass('public.users') IS NOT NULL"
}
