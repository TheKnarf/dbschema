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

### `time_limit`

Maximum time in seconds for the solving phase. If the solver exceeds this limit, enumeration stops and only the answer sets found so far are tested. Prevents runaway tests from poorly constrained programs.

```hcl
time_limit = 30
```

**Note:** The time limit is checked between answer sets, not during individual model computation. A single complex model that takes longer than the limit will still complete.

### `enum_mode`

Controls clingo's enumeration algorithm. Most useful for brave and cautious reasoning.

| Value      | Behavior |
|------------|----------|
| `auto`     | Automatic selection (default clingo behavior) |
| `bt`       | Backtrack-based enumeration |
| `record`   | Record-based enumeration |
| `domRec`   | Domain record-based enumeration |
| `brave`    | Compute brave consequences (atoms true in *at least one* answer set) |
| `cautious` | Compute cautious consequences (atoms true in *all* answer sets) |

#### Brave reasoning

Brave consequences are the union of all answer sets. If atom `pick(a)` appears in any answer set, it appears in the brave consequences. Produces a single test with all possible atoms.

```hcl
scenario "any_valid_assignment" {
  program = <<-ASP
    slot(morning; afternoon). task(a; b; c).
    1 { assign(T, S) : slot(S) } 1 :- task(T).
    #show assign/2.
  ASP
  enum_mode = "brave"
  map "assign" {
    sql = "INSERT INTO assignments (task, slot) VALUES ('{1}', '{2}')"
  }
}
```

#### Cautious reasoning

Cautious consequences are the intersection of all answer sets. Only atoms true in *every* answer set appear. Useful for verifying invariants that must hold regardless of non-deterministic choices.

```hcl
scenario "always_true_facts" {
  program = <<-ASP
    item(a; b; c).
    1 { pick(I) : item(I) } 1.
    #show item/1. #show pick/1.
  ASP
  enum_mode = "cautious"
  // Only item(a), item(b), item(c) appear — no pick atom is in every answer set
  map "item" {
    sql = "INSERT INTO items (name) VALUES ('{1}')"
  }
  assert_eq {
    query    = "SELECT COUNT(*)::text FROM items"
    expected = "3"
  }
}
```

### `project`

When `true`, clingo's `--project` flag collapses answer sets that are identical on the shown atoms (`#show` directives). This eliminates duplicate test runs caused by auxiliary atoms that don't affect the SQL being tested.

```hcl
scenario "bids" {
  program = <<-ASP
    bidder(alice; bob). amount(100; 200).
    role(admin; user).
    1 { bid(B, A) : bidder(B), amount(A) } 2.
    1 { assign(B, R) : role(R) } 1 :- bidder(B).
    #show bid/2.
  ASP
  project = true
  map "bid" { sql = "INSERT INTO bids (bidder, amount) VALUES ('{1}', {2})" }
}
```

### `opt_mode`

Controls optimization behavior. Valid values:

| Value    | Behavior |
|----------|----------|
| `opt`    | Find the single optimal model (skip intermediate improving models) |
| `optN`   | Enumerate all models at the optimal cost |
| `enum`   | Enumerate all models (default clingo behavior) |
| `ignore` | Ignore optimization statements |

Use with `#minimize` or `#maximize` directives in the ASP program.

```hcl
scenario "stress_bids" {
  program = <<-ASP
    bidder(alice; bob; charlie).
    amount(50; 100; 200; 500).
    1 { bid(B, A) : bidder(B), amount(A) } 3.
    :- bid(B, A1), bid(B, A2), A1 != A2.
    #maximize { 1,B : bid(B, _) }.
  ASP
  opt_mode = "optN"
  map "bid" { sql = "INSERT INTO bids (bidder, amount) VALUES ('{1}', {2})" }
}
```

### `focus`

List of ground atoms that must be true in every answer set. Only answer sets containing all focus atoms are enumerated — others are skipped.

Each string must be a valid ground atom that appears in the grounded program (e.g. `"pick(a)"`, `"bid(alice,100)"`).

```hcl
scenario "alice_bids_high" {
  program = <<-ASP
    bidder(alice; bob). amount(100; 200; 500).
    1 { bid(B, A) : bidder(B), amount(A) } 2.
    :- bid(B, A1), bid(B, A2), A1 != A2.
  ASP
  focus = ["bid(alice,500)"]
  map "bid" { sql = "INSERT INTO bids (bidder, amount) VALUES ('{1}', {2})" }
}
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

## Tips

### `#show` directives

Use `#show` to control which atoms are visible in test labels and available for mapping. Atoms not shown are still computed but won't appear in the test name or be matched by `map` blocks.

```asp
#show bid/2.       // only show bid atoms
#show pick/1.      // only show pick atoms
```

Combine `#show` with `project = true` to deduplicate answer sets that differ only in hidden auxiliary atoms.

### Advanced aggregates

Clingo supports aggregates for constraining generated test data:

```asp
// Exactly 2 bids per bidder
:- bidder(B), #count { A : bid(B, A) } != 2.

// Total bid amount between 100 and 500
:- #sum { A,B : bid(B, A) } < 100.
:- #sum { A,B : bid(B, A) } > 500.
```

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
