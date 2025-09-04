table "users" {
  schema = "public"
  column "id" {
    type = "serial"
    nullable = false
  }
  column "email" {
    type = "text"
    nullable = false
  }
  primary_key { columns = ["id"] }
}

policy "user_select" {
  schema  = "public"
  table   = "users"
  command = "select"
  roles   = ["app_user"]
  using   = "true"
}

test "policy" {
  assert = "SELECT EXISTS (SELECT 1 FROM pg_policy WHERE polname = 'user_select')"
}
