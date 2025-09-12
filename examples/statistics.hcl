table "orders" {
  column "region" { type = "text" }
  column "product" { type = "text" }
  primary_key { columns = ["region", "product"] }
}

statistics "orders_stats" {
  schema = "public"
  table  = "orders"
  columns = ["region", "product"]
  kinds   = ["ndistinct", "dependencies"]
  comment = "Multi-column statistics for orders"
}
