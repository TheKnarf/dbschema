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

## Running tests

Example command:

```bash
dbschema --input examples/table.hcl test \
  --dsn postgres://localhost:5432/mydb \
  --backend postgres \
  --apply \
  --name users_table
```

## Options

- `--dsn <string>`: Database connection string (falls back to `DATABASE_URL`).
- `--backend <postgres>`: Test backend (default: `postgres`).
- `--name <test_name>`: Run only matching tests; repeat to run multiple.
- `--apply`: Generate and apply migrations before running tests (Postgres only).
- `--create-db <name>`: Create a temporary database, run tests, then drop it.
- `--keep-db`: Keep the database created via `--create-db`.
- `--verbose`: Print SQL executed during apply and test phases.
