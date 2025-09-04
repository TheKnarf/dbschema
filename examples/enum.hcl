enum "status" {
  schema = "public"
  values = ["active", "disabled"]
}

test "status_enum" {
  assert = [
    "SELECT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'status')",
    "SELECT 'active'::status IS NOT NULL"
  ]
  assert_fail = [
    "SELECT 'unknown'::status"
  ]
}
