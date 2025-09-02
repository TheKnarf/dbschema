variable "tables" { default = ["users", "orders"] }

table "users" {
  schema = "public"
  column "updated_at" { type = "timestamptz" nullable = true }
}

table "orders" {
  schema = "public"
  column "updated_at" { type = "timestamptz" nullable = true }
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

trigger "upd" {
  for_each = var.tables
  name     = "set_updated_at_${each.value}"
  schema   = "public"
  table    = each.value
  timing   = "BEFORE"
  events   = ["UPDATE"]
  level    = "ROW"
  function = "set_updated_at"
}

test "for_each_triggers" {
  assert = "SELECT COUNT(*) = 2 FROM pg_trigger WHERE tgname IN ('set_updated_at_users','set_updated_at_orders')"
}
