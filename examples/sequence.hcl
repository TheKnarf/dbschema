table "users" {
  schema = "public"
  column "id" {
    type = "bigserial"
    nullable = false
  }
  primary_key { columns = ["id"] }
}

sequence "user_id_seq" {
  schema = "public"
  as = "bigint"
  increment = 1
  min_value = 1
  start = 1
  cache = 1
  cycle = false
}
