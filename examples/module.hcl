table "orders" {
  schema = "public"
  column "id" {
    type = "serial"
    nullable = false
  }
  column "updated_at" {
    type = "timestamptz"
    nullable = true
  }
  primary_key { columns = ["id"] }
}

module "timestamps" {
  source = "./modules/timestamps"
  schema = "public"
  table  = "orders"
  column = "updated_at"
}

test "module_trigger" {
  assert = "SELECT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'set_col_on_update')"
}
