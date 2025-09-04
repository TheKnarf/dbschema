variable "schema" { default = "public" }

function "f" {
  schema   = var.schema
  language = "plpgsql"
  returns  = "trigger"
  replace  = true
  body = <<-SQL
    BEGIN
      RETURN NEW;
    END;
  SQL
}

table "users" {
  schema = var.schema
  column "id" { type = "int" }
}

trigger "upd" {
  count    = 2
  name     = "set_updated_at_${count.index}"
  schema   = var.schema
  table    = "users"
  timing   = "BEFORE"
  events   = ["UPDATE"]
  level    = "ROW"
  function = "f"
}

test "count_triggers" {
  assert = [
    "SELECT COUNT(*) = 2 FROM pg_trigger WHERE tgname LIKE 'set_updated_at_%'"
  ]
  assert_fail = [
    "CREATE TRIGGER set_updated_at_0 BEFORE UPDATE ON public.users FOR EACH ROW EXECUTE FUNCTION f()"
  ]
}
