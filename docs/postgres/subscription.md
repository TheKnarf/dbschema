# Subscription

Creates a replication subscription.

```hcl
subscription "sub" {
  connection   = "host=localhost"
  publications = ["pub"]
  comment      = "subscribes to pub"
}
```

## Attributes
- `name` (label): subscription name.
- `connection` (string): libpq connection string.
- `publications` (list of string): publications to subscribe to.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
subscription "sub_main" {
  connection = "host=replica dbname=app user=rep password=secret"
  publications = ["pub_all"]
  comment     = "main subscription"
}
```
