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
  unique "users_email_key" { columns = ["email"] }
}

test "table" {
  assert = "SELECT to_regclass('public.users') IS NOT NULL"
}
