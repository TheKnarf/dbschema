variable "schema" { default = "public" }
variable "table" {}
variable "column" { default = "updated_at" }

function "set_updated_col" {
  schema   = var.schema
  language = "plpgsql"
  returns  = "trigger"
  replace  = true
  body = <<-SQL
    BEGIN
      NEW.${var.column} = now();
      RETURN NEW;
    END;
  SQL
}

trigger "set_col_on_update" {
  schema   = var.schema
  table    = var.table
  timing   = "BEFORE"
  events   = ["UPDATE"]
  level    = "ROW"
  function = "set_updated_col"
}
