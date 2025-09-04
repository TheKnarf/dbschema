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
  # Positive assert
  assert = [
    "SELECT to_regclass('public.users') IS NOT NULL"
  ]
}

# Negative test: unique index prevents duplicates
test "table_unique_enforced" {
  setup = [
    "INSERT INTO public.users(email) VALUES ('dup')"
  ]
  assert_fail = [
    "INSERT INTO public.users(email) VALUES ('dup')"
  ]
}
