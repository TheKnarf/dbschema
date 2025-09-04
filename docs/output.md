# Output

`output` blocks expose values from a configuration or module so they can be consumed elsewhere.

```hcl
variable "count" { default = 1 }

output "count" {
  value = var.count
}
```

Outputs are printed after evaluation and can be referenced by parent modules.
