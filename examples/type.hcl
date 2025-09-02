type "address" {
  field "street" { type = "text" }
  field "zip" { type = "int" }
}

test "address_type" {
  assert = "SELECT ROW('road',12345)::address IS NOT NULL"
}
