enum "StatusType" {
  schema = "public"
  values = ["active", "disabled"]
}

table "status_usage_test" {
  schema = "public"

  column "id" {
    type = "text"
    nullable = false
  }

  column "status" {
    type = "StatusType"
    nullable = false
  }

  primary_key { columns = ["id"] }

}

test "status_enum" {
  assert = [
    "SELECT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'StatusType')",
    "SELECT 'active'::\"StatusType\" IS NOT NULL",
    "WITH inserted AS (INSERT INTO public.status_usage_test (id, status) VALUES ('usage-test', 'active'::\"StatusType\") RETURNING 1) SELECT EXISTS (SELECT 1 FROM inserted)"
  ]
  assert_fail = [
    "SELECT 'unknown'::\"StatusType\""
  ]
}
