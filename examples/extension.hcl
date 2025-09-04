extension "pgcrypto" {}

test "pgcrypto" {
  assert = [
    "SELECT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'pgcrypto')",
    "SELECT gen_random_uuid() IS NOT NULL"
  ]
}
