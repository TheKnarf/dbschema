schema "analytics" {}

test "schema" {
  assert = "SELECT EXISTS (SELECT 1 FROM information_schema.schemata WHERE schema_name = 'analytics')"
}
