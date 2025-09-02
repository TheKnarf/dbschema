variable "cols" {
  default = {
    id   = { type = "serial", nullable = false }
    name = { type = "text",   nullable = true }
  }
}

table "users" {
  dynamic "column" {
    for_each = var.cols
    labels   = [each.key]
    content {
      type     = each.value.type
      nullable = each.value.nullable
    }
  }
}

test "dynamic_columns" {
  assert = "SELECT COUNT(*) = 2 FROM information_schema.columns WHERE table_schema='public' AND table_name='users'"
}
