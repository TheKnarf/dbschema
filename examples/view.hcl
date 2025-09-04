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

view "active_users" {
  schema = "public"
  replace = true
  sql = <<-SQL
    SELECT id, email FROM public.users
  SQL
}

test "view" {
  setup = ["INSERT INTO public.users (email) VALUES ('test@example.com')"]
  assert = "SELECT COUNT(*) = 1 FROM public.active_users WHERE email = 'test@example.com'"
}
