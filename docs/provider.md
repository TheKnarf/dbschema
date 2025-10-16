# Providers

Providers describe which database backend dbschema should target. They are defined with
`provider` blocks in HCL and follow a Terraform-like syntax. Each block's label indicates
the provider type.

```hcl
provider "postgres" {
  version = "16"
}
```

Currently only the `postgres` provider is supported. The `version` attribute is optional and
can be used to declare the target Postgres major version. Future releases will use this
information to enable version-specific validations.

Defining a provider block is optional. When omitted, dbschema assumes a default Postgres
provider without a version constraint.
