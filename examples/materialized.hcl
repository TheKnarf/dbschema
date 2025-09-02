materialized "user_counts" {
  schema = "public"
  with_data = true
  sql = <<-SQL
    SELECT 1 as id
  SQL
}

test "materialized_view" {
  assert = "SELECT id = 1 FROM public.user_counts"
}
