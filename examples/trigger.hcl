table "users" {
  schema = "public"
  column "id" { type = "serial" nullable = false }
  column "updated_at" { type = "timestamptz" nullable = true }
  primary_key { columns = ["id"] }
}

function "set_updated_at" {
  schema   = "public"
  language = "plpgsql"
  returns  = "trigger"
  replace  = true
  body = <<-SQL
    BEGIN
      NEW.updated_at = now();
      RETURN NEW;
    END;
  SQL
}

trigger "users_updated_at" {
  schema   = "public"
  table    = "users"
  timing   = "BEFORE"
  events   = ["UPDATE"]
  level    = "ROW"
  function = "set_updated_at"
}

test "trigger" {
  assert = "SELECT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'users_updated_at')"
}
