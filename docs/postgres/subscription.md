# Subscription

Creates a replication subscription.

```hcl
subscription "sub" {
  connection   = "host=localhost"
  publications = ["pub"]
}
```

## Attributes
- `name` (label): subscription name.
- `connection` (string): libpq connection string.
- `publications` (list of string): publications to subscribe to.
- `comment` (string, optional): documentation comment.
