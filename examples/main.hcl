# Root example: defines a function and a trigger directly,
# and also uses a module that creates the same pattern for another table.

extension "pgcrypto" {}

variable "schema" { default = "public" }

locals {
  updated_at_column = "updated_at"
}

enum "user_status" {
  schema = var.schema
  values = ["active", "disabled"]
}

table "users" {
  schema = var.schema
  if_not_exists = true

  column "id" {
    type = "serial"
    nullable = false
  }
  column "email" {
    type = "text"
    nullable = false
  }
  column "status" {
    type = "user_status"
    nullable = false
  }
  column "updated_at" {
    type = "timestamptz"
  }

  primary_key { columns = ["id"] }
  unique "users_email_key" { columns = ["email"] }
  index  "users_updated_at_idx" { columns = ["updated_at"] }
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

view "active_users" {
  schema  = var.schema
  replace = true
  sql = <<-SQL
    SELECT id, email, updated_at
    FROM public.users
    WHERE updated_at IS NOT NULL
  SQL
}

materialized "users_by_domain" {
  schema = var.schema
  with_data = true
  sql = <<-SQL
    SELECT split_part(email, '@', 2) as domain, count(*) as users
    FROM public.users
    GROUP BY domain
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

test "users_updated_at_sets_timestamp" {
  setup = [
    "CREATE TABLE IF NOT EXISTS public.users(id serial primary key, updated_at timestamptz)",
    "CREATE OR REPLACE FUNCTION public.set_updated_at() RETURNS trigger LANGUAGE plpgsql AS $$\nBEGIN\n  NEW.updated_at = now();\n  RETURN NEW;\nEND;\n$$;",
    "DO $$\nBEGIN\n  IF NOT EXISTS (\n    SELECT 1 FROM pg_trigger tg\n    JOIN pg_class c ON c.oid = tg.tgrelid\n    JOIN pg_namespace n ON n.oid = c.relnamespace\n    WHERE tg.tgname = 'users_updated_at'\n      AND n.nspname = 'public'\n      AND c.relname = 'users'\n  ) THEN\n    CREATE TRIGGER users_updated_at\n    BEFORE UPDATE ON public.users\n    FOR EACH ROW\n    EXECUTE FUNCTION public.set_updated_at();\n  END IF;\nEND$$;",
    "INSERT INTO public.users(updated_at) VALUES ('1970-01-01 00:00:00+00')",
    "UPDATE public.users SET id = id"
  ]
  assert = "SELECT updated_at > '1970-01-01 00:00:00+00' FROM public.users LIMIT 1"
}
