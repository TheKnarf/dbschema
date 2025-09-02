extension "pgcrypto" {}

test "pgcrypto" {
  assert = "SELECT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'pgcrypto')"
}
