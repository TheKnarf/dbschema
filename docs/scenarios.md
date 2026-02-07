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

### `seed`

Random seed for the clingo solver. Controls answer set enumeration order for reproducible test runs.

When omitted, a random seed is generated automatically. When a scenario test fails, the seed is included in the error message so you can reproduce the exact run.

```hcl
seed = 42
```

Override all scenario seeds from the CLI:

```bash
dbschema test --seed 42 --dsn postgres://... --apply
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

## Multi-step scenarios

Multi-step scenarios use clingo's `#program step(t)` directive to model temporal workflows. Step 1 might create entities, step 2 performs actions, step 3 verifies results. Each step commits its own transaction so data accumulates across steps.

### `steps` attribute

Number of step parts to ground. When set, clingo grounds `base` + `step(1)` through `step(N)` together, then solves once.

```hcl
steps = 2
```

### `step {}` blocks

Ordered blocks that execute sequentially for each answer set. Each step block supports the same elements as the top level: `setup`, `map`, `check`, `assert_eq`, `assert_snapshot`.

```hcl
step {
  map "bid" {
    sql = "INSERT INTO bids (user_name, amount) VALUES ('{1}', {2})"
  }
  check "bids_exist" {
    assert = ["SELECT COUNT(*) > 0 FROM bids"]
  }
}

step {
  map "winner" {
    sql = "UPDATE users SET won = true WHERE name = '{1}'"
  }
  assert_eq {
    query = "SELECT COUNT(*)::text FROM users WHERE won = true"
    expected = "1"
  }
}
```

### ASP `#program step(t)` usage

The ASP program uses `#program step(t).` to declare rules that are grounded for each step value. Use conditions on `t` to control which atoms appear at which step:

```asp
#program base.
user(alice; bob).

#program step(t).
bid(U, 100) :- user(U), t == 1.
winner(U) :- bid(U, A), A = #max { V : bid(_, V) }, t == 2.
```

### Multi-step execution order

For each answer set:

1. **Base transaction**: begin tx → top-level `setup` SQL → top-level `map` blocks → commit
2. **For each `step` block** (in declaration order):
   - Begin transaction
   - Run step `setup` SQL
   - Execute step `map` blocks against the full atom set
   - Run step `assert_eq` / `assert_snapshot`
   - Run step `check` blocks
   - Commit transaction
3. **Final verification transaction**: begin tx → top-level `assert_eq` / `assert_snapshot` → top-level `check` blocks → global `invariant` blocks → rollback (read-only)
4. **Teardown**: execute `teardown` SQL (cleanup committed data)

### Example

```hcl
scenario "auction_workflow" {
  program = <<-ASP
    #program base.
    user(alice; bob).

    #program step(t).
    bid(U, 100) :- user(U), t == 1.
    winner(U) :- bid(U, A), A = #max { V : bid(_, V) }, t == 2.
  ASP

  steps = 2

  setup = ["INSERT INTO items (name) VALUES ('Widget')"]

  step {
    map "bid" {
      sql = "INSERT INTO bids (user_name, amount) VALUES ('{1}', {2})"
    }
    check "bids_exist" {
      assert = ["SELECT COUNT(*) > 0 FROM bids"]
    }
  }

  step {
    map "winner" {
      sql = "UPDATE users SET won = true WHERE name = '{1}'"
    }
    assert_eq {
      query = "SELECT COUNT(*)::text FROM users WHERE won = true"
      expected = "1"
    }
  }

  teardown = [
    "DELETE FROM bids",
    "DELETE FROM items",
  ]
}
```

**Note:** Teardown is essential for multi-step scenarios since data is committed (not rolled back).

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
