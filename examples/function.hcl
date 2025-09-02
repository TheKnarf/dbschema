function "now_utc" {
  schema   = "public"
  language = "sql"
  returns  = "timestamptz"
  replace  = true
  body     = "SELECT now()"
}

test "now_utc" {
  assert = "SELECT now_utc() IS NOT NULL"
}
