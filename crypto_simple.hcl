# Demonstrate cryptographic functions
table "users" {
  schema = "public"

  column "id" {
    type = "serial"
    nullable = false
  }

  # Store hashed password using SHA256
  column "password_hash" {
    type = "text"
    nullable = false
    default = "${sha256(test)}"
  }

  # Store password salt using MD5
  column "salt" {
    type = "text"
    nullable = false
    default = "${md5(salt)}"
  }

  primary_key { columns = ["id"] }
}

# Demonstrate base64 functions
table "credentials" {
  schema = "public"

  column "id" {
    type = "serial"
    nullable = false
  }

  column "encoded_data" {
    type = "text"
    nullable = false
    default = "${base64encode(data)}"
  }

  column "decoded_data" {
    type = "text"
    nullable = false
    default = "${base64decode(dGVzdA)}"
  }

  primary_key { columns = ["id"] }
}