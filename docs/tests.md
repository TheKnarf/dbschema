# Tests

`test` blocks execute SQL to verify that generated resources behave as expected. Each test can define optional `setup` and `teardown` statements, `assert` queries that must succeed, and `assert_fail` queries that should fail.

```hcl
test "users_table" {
  setup = ["INSERT INTO public.users(email) VALUES ('a@b.com')"]
  assert = [
    "SELECT COUNT(*) = 1 FROM public.users"
  ]
  assert_fail = [
    "INSERT INTO public.users(email) VALUES ('a@b.com')"
  ]
}
```

When run against Postgres, each test executes inside a transaction and rolls back automatically.
