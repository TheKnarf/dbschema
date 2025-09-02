enum "status" {
  schema = "public"
  values = ["active", "disabled"]
}

test "status_enum" {
  assert = "SELECT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'status')"
}
