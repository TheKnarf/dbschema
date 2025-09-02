variable "count" {
  type = "number"
  validation {
    condition     = var.count > 0
    error_message = "count must be positive"
  }
}

output "count" { value = var.count }
