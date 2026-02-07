# Scenarios (ASP-driven testing)

`scenario` blocks use [Answer Set Programming](https://en.wikipedia.org/wiki/Answer_set_programming) (via [clingo](https://potassco.org/clingo/)) to generate combinatorial test cases from a logic program. Each answer set becomes a separate test.

Requires building with `--features scenario` and having `clingo` on `PATH`.

## Basic example

```hcl
scenario "bid_processing" {
  program = <<-ASP
    bidder(alice; bob).
    amount(100; 200).
    1 { bid(B, A) : bidder(B), amount(A) } 2.
    :- bid(B, A1), bid(B, A2), A1 != A2.
  ASP

  setup = [
    "INSERT INTO items (name, start_at, end_at) VALUES ('Test', NOW() - interval '1 hour', NOW() + interval '1 hour')",
  ]

  map "bid" {
    sql = "INSERT INTO bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), '{1}', {2})"
  }

  runs = 10
}
```

## Attributes reference

### `program` (required)

ASP logic program as a heredoc string. Defines atoms, choice rules, and constraints. Each stable model (answer set) produced by clingo becomes a test case.

### `setup`

SQL statements executed once at the start of each answer set, before any map SQL.

```hcl
setup = [
  "INSERT INTO items (name) VALUES ('Test')",
]
```

### `map` blocks

Named blocks that map ASP atoms to SQL. The block label must match an atom name in the program. Arguments are substituted via `{1}`, `{2}`, etc. (1-based).

```hcl
map "bid" {
  sql = "INSERT INTO bids (bidder, amount) VALUES ('{1}', {2})"
}
```

**Important:** Use `{1}` not `${1}` — the `${}` syntax is HCL template interpolation and would be evaluated at parse time.

#### `order_by`

Sort matching atoms by a specific argument index (1-based) before executing. Numeric values are sorted numerically; others lexicographically.

```hcl
map "bid" {
  sql      = "INSERT INTO bids (bidder, amount) VALUES ('{1}', {2})"
  order_by = 2
}
```

### `runs`

Maximum number of answer sets to execute. Defaults to `0` (all answer sets).

```hcl
runs = 10
```

### `check` blocks

Scenario-scoped invariants. Each check contains `assert` queries that must return true after every answer set. Checks run after `assert_eq` / `assert_snapshot` but before global invariants.

```hcl
check "highest_bid_approved" {
  assert = [
    "SELECT br.status = 'approved' FROM bid_results br JOIN bids b ON b.id = br.bid_id WHERE b.amount = (SELECT MAX(amount) FROM bids)",
  ]
}
```

### `expect_error`

When `true`, the test passes if any map SQL raises an error (e.g. a constraint violation). If all SQL succeeds, the test fails.

```hcl
expect_error = true
```

### `assert_eq`

Inline equality assertion. Executes `query`, takes the first column of the first row, and compares it as a string to `expected`.

```hcl
assert_eq {
  query    = "SELECT COUNT(*)::text FROM bids"
  expected = "3"
}
```

### `assert_snapshot`

Multi-row snapshot assertion. Compares all rows and columns returned by `query` against the expected `rows` array. Row count and column count must match exactly.

```hcl
assert_snapshot {
  query = "SELECT status::text FROM bid_results ORDER BY id"
  rows = [
    ["approved"],
    ["rejected"],
  ]
}
```

### `params`

Inject `#const` values into the ASP program. Each key-value pair is prepended as `#const key=value.` before grounding.

```hcl
params = {
  max_bids = "3"
  min_amount = "100"
}
```

In the ASP program, reference them as constants:

```asp
amount(1..max_bids).
:- bid(_, A), A < min_amount.
```

### `teardown`

SQL statements executed after the transaction is rolled back. Useful for cleaning up data that was committed outside the transaction (e.g. when using `expect_error`).

```hcl
teardown = [
  "DELETE FROM bid_results",
  "DELETE FROM bids",
]
```

## Execution order

For each answer set produced by clingo:

1. **Begin transaction**
2. **Setup** — execute `setup` SQL statements in order
3. **Maps** — for each `map` block (in declaration order):
   - Find all matching atoms in the answer set
   - Sort atoms by `order_by` index (or alphabetically if unset)
   - Substitute arguments and execute SQL
   - If `expect_error = true` and SQL fails: pass immediately
4. **Assertions** — run `assert_eq` and `assert_snapshot` checks
5. **Scenario checks** — run `check` block assertions
6. **Global invariants** — run any `invariant` blocks defined in the file
7. **Rollback transaction**
8. **Teardown** — execute `teardown` SQL (outside the transaction)

## Labels

Each generated test is automatically labeled with the atoms from its answer set:

```
scenario[0: bid(alice,100), bid(bob,200)]
scenario[1: bid(alice,200), bid(bob,100)]
```

Up to 5 atoms are shown; larger sets display `... (N total)`.

## Running scenarios

```bash
dbschema --input examples/bidding.hcl test \
  --dsn postgres://postgres:postgres@localhost:5432/postgres \
  --create-db test_db --apply
```

Scenarios require the `scenario` cargo feature:

```bash
cargo build --features scenario
```
