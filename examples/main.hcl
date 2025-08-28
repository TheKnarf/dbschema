# Root example: defines a function and a trigger directly,
# and also uses a module that creates the same pattern for another table.

extension "pgcrypto" {}

variable "schema" { default = "public" }

locals {
  updated_at_column = "updated_at"
}

function "set_updated_at" {
  schema   = var.schema
  language = "plpgsql"
  returns  = "trigger"
  replace  = true
  body = <<-SQL
    BEGIN
      NEW.${local.updated_at_column} = now();
      RETURN NEW;
    END;
  SQL
}

trigger "users_updated_at" {
  schema   = var.schema
  table    = "users"
  timing   = "BEFORE"
  events   = ["UPDATE"]
  level    = "ROW"
  function = "set_updated_at"
}

module "orders_timestamps" {
  source = "./modules/timestamps"
  schema = var.schema
  table  = "orders"
  column = "updated_at"
}
