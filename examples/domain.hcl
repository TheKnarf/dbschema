domain "email" {
  type = "text"
  check = "VALUE ~* '^[^@]+@[^@]+$'"
}

test "email_domain" {
  assert = "SELECT 'user@example.com'::email IS NOT NULL"
}
