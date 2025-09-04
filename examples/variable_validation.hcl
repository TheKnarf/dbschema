variable "count" {
  type    = "number"
  default = 1
  validation {
    condition     = var.count > 0
    error_message = "count must be positive"
  }
}

output "count" { value = var.count }
