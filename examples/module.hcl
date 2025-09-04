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
  assert = [
    "SELECT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'set_col_on_update')"
  ]
}

test "module_behavior" {
  setup = [
    "INSERT INTO public.orders DEFAULT VALUES",
    "UPDATE public.orders SET updated_at = updated_at"
  ]
  assert = [
    "SELECT COUNT(*) = 1 FROM public.orders WHERE updated_at IS NOT NULL"
  ]
}
