# Modules

Modules allow reusing groups of resources. A module points to a directory containing a `main.hcl` file and passes in variables.

```hcl
module "timestamps" {
  source = "./modules/timestamps"
  schema = "public"
  table  = "orders"
  column = "updated_at"
}
```

Inside the module, variables provided by the caller are accessible via `var.<name>`. Modules can themselves declare outputs to expose values back to the parent configuration.
