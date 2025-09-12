# Modules and Output

## Modules

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

## Output

`output` blocks expose values from a configuration or module so they can be consumed elsewhere.

```hcl
variable "count" { default = 1 }

output "count" {
  value = var.count
}
```

Outputs are printed after evaluation and can be referenced by parent modules.
