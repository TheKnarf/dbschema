variable "users" {
  default = [
    "alice",
    "bob"
  ]
}

output "greetings" {
  value = [for u in var.users : "hi ${upper(u)}"]
}

variable "premium" {
  default = false
}

output "plan" {
  value = var.premium ? "pro" : "basic"
}
