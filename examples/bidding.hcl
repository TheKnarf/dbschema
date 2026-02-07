# Bidding System
#
# An auction platform where users bid on items within a time window.
# Features automatic bid extension (snipe protection), autobids that
# outbid on behalf of a user up to a max amount, and real-time
# notifications via pg_notify when bids are approved or rejected.
#
# Schema: items, bids, autobids, bid_results, auction_status (view)
# Triggers: check_bid_timing (enforces auction window), process_bid
#           (approves/rejects and applies autobids), notify_bid_result
#
# Run tests (requires a running Postgres instance):
#
#   dbschema --input examples/bidding.hcl test \
#     --dsn postgres://postgres:postgres@localhost:5432/postgres \
#     --create-db bidding_test --apply
#
# Or via the justfile (starts Postgres via Docker Compose):
#
#   just example-test file=examples/bidding.hcl

enum "bid_status" {
  schema = "public"
  values = ["approved", "rejected"]
}

table "items" {
  schema = "public"

  column "id" {
    type     = "serial"
    nullable = false
  }

  column "name" {
    type     = "text"
    nullable = false
  }

  column "start_at" {
    type     = "timestamptz"
    nullable = false
  }

  column "end_at" {
    type     = "timestamptz"
    nullable = false
  }

  primary_key { columns = ["id"] }

  check "valid_time_range" {
    expression = "end_at > start_at"
  }
}

table "bids" {
  schema = "public"

  column "id" {
    type     = "serial"
    nullable = false
  }

  column "item_id" {
    type     = "integer"
    nullable = false
  }

  column "bidder" {
    type     = "text"
    nullable = false
  }

  column "amount" {
    type     = "integer"
    nullable = false
  }

  column "created_at" {
    type     = "timestamptz"
    nullable = false
    default  = "now()"
  }

  primary_key { columns = ["id"] }

  foreign_key {
    columns = ["item_id"]
    ref {
      table   = "items"
      columns = ["id"]
    }
    on_delete = "CASCADE"
  }

  check "positive_amount" {
    expression = "amount > 0"
  }
}

table "autobids" {
  schema = "public"

  column "id" {
    type     = "serial"
    nullable = false
  }

  column "item_id" {
    type     = "integer"
    nullable = false
  }

  column "bidder" {
    type     = "text"
    nullable = false
  }

  column "max_amount" {
    type     = "integer"
    nullable = false
  }

  primary_key { columns = ["id"] }

  foreign_key {
    columns = ["item_id"]
    ref {
      table   = "items"
      columns = ["id"]
    }
    on_delete = "CASCADE"
  }

  check "positive_max_amount" {
    expression = "max_amount > 0"
  }
}

index "autobids_item_bidder_key" {
  table   = "autobids"
  columns = ["item_id", "bidder"]
  unique  = true
}

table "bid_results" {
  schema = "public"

  column "id" {
    type     = "serial"
    nullable = false
  }

  column "bid_id" {
    type     = "integer"
    nullable = false
  }

  column "item_id" {
    type     = "integer"
    nullable = false
  }

  column "status" {
    type     = "bid_status"
    nullable = false
  }

  column "credited_to" {
    type     = "text"
    nullable = false
  }

  primary_key { columns = ["id"] }

  foreign_key {
    columns = ["bid_id"]
    ref {
      table   = "bids"
      columns = ["id"]
    }
    on_delete = "CASCADE"
  }

  foreign_key {
    columns = ["item_id"]
    ref {
      table   = "items"
      columns = ["id"]
    }
    on_delete = "CASCADE"
  }
}

index "bid_results_item_status_idx" {
  table   = "bid_results"
  columns = ["item_id", "status"]
}

view "auction_status" {
  schema  = "public"
  replace = true
  sql     = <<-SQL
    SELECT
      i.id,
      i.name,
      i.start_at,
      GREATEST(
        i.end_at,
        (SELECT MAX(b.created_at) + interval '30 seconds'
           FROM public.bids b
          WHERE b.item_id = i.id)
      ) AS effective_end,
      COALESCE(approved.cnt, 0) AS approved_bid_count,
      approved.highest_bid
    FROM public.items i
    LEFT JOIN LATERAL (
      SELECT
        COUNT(*)   AS cnt,
        MAX(b.amount) AS highest_bid
      FROM public.bid_results br
      JOIN public.bids b ON b.id = br.bid_id
      WHERE br.item_id = i.id
        AND br.status = 'approved'
    ) approved ON true
  SQL
}

function "check_bid_timing" {
  schema   = "public"
  language = "plpgsql"
  returns  = "trigger"
  replace  = true
  body     = <<-SQL
    DECLARE
      item_start    timestamptz;
      item_end      timestamptz;
      latest_bid_at timestamptz;
      effective_end timestamptz;
    BEGIN
      SELECT i.start_at, i.end_at
        INTO item_start, item_end
        FROM public.items i
       WHERE i.id = NEW.item_id;

      IF NOW() < item_start THEN
        RAISE EXCEPTION 'Auction has not started yet';
      END IF;

      SELECT MAX(b.created_at)
        INTO latest_bid_at
        FROM public.bids b
       WHERE b.item_id = NEW.item_id;

      effective_end := GREATEST(
        item_end,
        latest_bid_at + interval '30 seconds'
      );

      IF NOW() > effective_end THEN
        RAISE EXCEPTION 'Auction has ended';
      END IF;

      RETURN NEW;
    END;
  SQL
}

trigger "bids_check_timing" {
  schema   = "public"
  table    = "bids"
  timing   = "BEFORE"
  events   = ["INSERT"]
  level    = "ROW"
  function = "check_bid_timing"
}

function "process_bid" {
  schema   = "public"
  language = "plpgsql"
  returns  = "trigger"
  replace  = true
  body     = <<-SQL
    DECLARE
      current_highest integer;
      autobid_bidder  text;
    BEGIN
      SELECT COALESCE(MAX(b.amount), 0)
        INTO current_highest
        FROM public.bid_results br
        JOIN public.bids b ON b.id = br.bid_id
       WHERE br.item_id = NEW.item_id
         AND br.status = 'approved';

      IF NEW.amount <= current_highest THEN
        INSERT INTO public.bid_results (bid_id, item_id, status, credited_to)
        VALUES (NEW.id, NEW.item_id, 'rejected', NEW.bidder);
      ELSE
        SELECT ab.bidder
          INTO autobid_bidder
          FROM public.autobids ab
         WHERE ab.item_id = NEW.item_id
           AND ab.bidder <> NEW.bidder
           AND ab.max_amount > NEW.amount
         ORDER BY ab.max_amount DESC
         LIMIT 1;

        IF autobid_bidder IS NOT NULL THEN
          INSERT INTO public.bid_results (bid_id, item_id, status, credited_to)
          VALUES (NEW.id, NEW.item_id, 'approved', autobid_bidder);
        ELSE
          INSERT INTO public.bid_results (bid_id, item_id, status, credited_to)
          VALUES (NEW.id, NEW.item_id, 'approved', NEW.bidder);
        END IF;
      END IF;

      RETURN NEW;
    END;
  SQL
}

trigger "bids_process" {
  schema   = "public"
  table    = "bids"
  timing   = "AFTER"
  events   = ["INSERT"]
  level    = "ROW"
  function = "process_bid"
}

function "notify_bid_result" {
  schema   = "public"
  language = "plpgsql"
  returns  = "trigger"
  replace  = true
  body     = <<-SQL
    BEGIN
      PERFORM pg_notify('bid_result', json_build_object(
        'item_id',     NEW.item_id,
        'bid_id',      NEW.bid_id,
        'status',      NEW.status,
        'credited_to', NEW.credited_to
      )::text);
      RETURN NEW;
    END;
  SQL
}

trigger "bid_results_notify" {
  schema   = "public"
  table    = "bid_results"
  timing   = "AFTER"
  events   = ["INSERT"]
  level    = "ROW"
  function = "notify_bid_result"
}

# --- Invariants ---
# These run after every test to enforce cross-cutting properties.

invariant "every_bid_has_a_result" {
  assert = [
    "SELECT COUNT(*) = 0 FROM public.bids b WHERE NOT EXISTS (SELECT 1 FROM public.bid_results br WHERE br.bid_id = b.id)",
  ]
}

invariant "no_non_positive_amounts" {
  assert = [
    "SELECT COUNT(*) = 0 FROM public.bids WHERE amount <= 0",
  ]
}

# --- Tests ---
# Note: Each test runs in its own transaction that rolls back, but serial
# sequences are not transactional. We use subqueries instead of hardcoded IDs
# to look up rows by their business-key columns (bidder, amount).

test "first_bid_approved" {
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Vintage Watch', NOW() - interval '1 hour', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
  ]
  assert = [
    "SELECT COUNT(*) = 1 FROM public.bid_results br JOIN public.bids b ON b.id = br.bid_id WHERE b.bidder = 'alice' AND b.amount = 100",
  ]
  assert_eq {
    query    = "SELECT br.status::text FROM public.bid_results br JOIN public.bids b ON b.id = br.bid_id WHERE b.bidder = 'alice' AND b.amount = 100"
    expected = "approved"
  }
  assert_eq {
    query    = "SELECT br.credited_to FROM public.bid_results br JOIN public.bids b ON b.id = br.bid_id WHERE b.bidder = 'alice' AND b.amount = 100"
    expected = "alice"
  }
}

test "bid_at_or_below_highest_rejected" {
  for_each = {
    lower = "50"
    equal = "100"
  }
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Vintage Watch', NOW() - interval '1 hour', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'bob', ${each.value})",
  ]
  assert_eq {
    query    = "SELECT br.status::text FROM public.bid_results br JOIN public.bids b ON b.id = br.bid_id WHERE b.bidder = 'bob'"
    expected = "rejected"
  }
  assert_eq {
    query    = "SELECT br.credited_to FROM public.bid_results br JOIN public.bids b ON b.id = br.bid_id WHERE b.bidder = 'bob'"
    expected = "bob"
  }
}

test "higher_bid_approved" {
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Vintage Watch', NOW() - interval '1 hour', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'bob', 200)",
  ]
  assert_eq {
    query    = "SELECT br.status::text FROM public.bid_results br JOIN public.bids b ON b.id = br.bid_id WHERE b.bidder = 'bob' AND b.amount = 200"
    expected = "approved"
  }
  assert_eq {
    query    = "SELECT br.credited_to FROM public.bid_results br JOIN public.bids b ON b.id = br.bid_id WHERE b.bidder = 'bob' AND b.amount = 200"
    expected = "bob"
  }
}

test "autobid_outbids_new_bidder" {
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Vintage Watch', NOW() - interval '1 hour', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
    "INSERT INTO public.autobids (item_id, bidder, max_amount) VALUES (currval('items_id_seq'), 'charlie', 500)",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'bob', 200)",
  ]
  assert_eq {
    query    = "SELECT br.status::text FROM public.bid_results br JOIN public.bids b ON b.id = br.bid_id WHERE b.bidder = 'bob' AND b.amount = 200"
    expected = "approved"
  }
  assert_eq {
    query    = "SELECT br.credited_to FROM public.bid_results br JOIN public.bids b ON b.id = br.bid_id WHERE b.bidder = 'bob' AND b.amount = 200"
    expected = "charlie"
  }
}

test "autobid_exceeded_original_wins" {
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Vintage Watch', NOW() - interval '1 hour', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
    "INSERT INTO public.autobids (item_id, bidder, max_amount) VALUES (currval('items_id_seq'), 'charlie', 150)",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'bob', 200)",
  ]
  assert_eq {
    query    = "SELECT br.status::text FROM public.bid_results br JOIN public.bids b ON b.id = br.bid_id WHERE b.bidder = 'bob' AND b.amount = 200"
    expected = "approved"
  }
  assert_eq {
    query    = "SELECT br.credited_to FROM public.bid_results br JOIN public.bids b ON b.id = br.bid_id WHERE b.bidder = 'bob' AND b.amount = 200"
    expected = "bob"
  }
}

test "multiple_autobids_highest_wins" {
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Vintage Watch', NOW() - interval '1 hour', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
    "INSERT INTO public.autobids (item_id, bidder, max_amount) VALUES (currval('items_id_seq'), 'charlie', 300)",
    "INSERT INTO public.autobids (item_id, bidder, max_amount) VALUES (currval('items_id_seq'), 'diana', 500)",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'bob', 200)",
  ]
  assert_eq {
    query    = "SELECT br.status::text FROM public.bid_results br JOIN public.bids b ON b.id = br.bid_id WHERE b.bidder = 'bob' AND b.amount = 200"
    expected = "approved"
  }
  assert_eq {
    query    = "SELECT br.credited_to FROM public.bid_results br JOIN public.bids b ON b.id = br.bid_id WHERE b.bidder = 'bob' AND b.amount = 200"
    expected = "diana"
  }
}

test "rejected_bid_still_recorded" {
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Vintage Watch', NOW() - interval '1 hour', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'bob', 50)",
  ]
  assert_snapshot {
    query = "SELECT status::text FROM public.bid_results ORDER BY status"
    rows = [
      ["approved"],
      ["rejected"],
    ]
  }
}

# --- Timing Tests ---

test "bid_before_auction_starts_rejected" {
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Future Item', NOW() + interval '1 hour', NOW() + interval '2 hours')",
  ]
  assert_error {
    sql              = "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)"
    message_contains = "Auction has not started yet"
  }
}

test "bid_after_auction_ends_rejected" {
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Expired Item', NOW() - interval '2 hours', NOW() - interval '1 hour')",
  ]
  assert_error {
    sql              = "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)"
    message_contains = "Auction has ended"
  }
}

test "bid_during_open_auction_accepted" {
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Live Item', NOW() - interval '1 hour', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
  ]
  assert = [
    "SELECT COUNT(*) = 1 FROM public.bids WHERE bidder = 'alice'",
  ]
}

test "extension_allows_bid_after_end_at" {
  # end_at is nominally 5 seconds ago, but there is a recent bid (created_at = NOW())
  # so effective_end = GREATEST(NOW()-5s, NOW()+30s) = NOW()+30s → bid is allowed
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Extended Item', NOW() - interval '1 hour', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
    "UPDATE public.items SET end_at = NOW() - interval '5 seconds' WHERE id = currval('items_id_seq')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'bob', 200)",
  ]
  assert = [
    "SELECT COUNT(*) = 2 FROM public.bids WHERE item_id = currval('items_id_seq')",
    "SELECT br.status = 'approved' FROM public.bid_results br JOIN public.bids b ON b.id = br.bid_id WHERE b.bidder = 'bob' AND b.amount = 200",
  ]
}

test "extension_does_not_apply_with_old_bids" {
  # end_at is 1 minute ago, latest bid was 2 minutes ago
  # effective_end = GREATEST(NOW()-1min, NOW()-2min+30s) = GREATEST(NOW()-60s, NOW()-90s) = NOW()-60s
  # NOW() > effective_end → rejected
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Closed Item', NOW() - interval '1 hour', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
    "UPDATE public.items SET end_at = NOW() - interval '1 minute' WHERE id = currval('items_id_seq')",
    "UPDATE public.bids SET created_at = NOW() - interval '2 minutes' WHERE item_id = currval('items_id_seq')",
  ]
  assert_fail = [
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'bob', 200)",
  ]
}

# --- View Tests ---

test "view_item_with_no_bids" {
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Empty Auction', NOW() - interval '1 hour', NOW() + interval '1 hour')",
  ]
  assert_eq {
    query    = "SELECT name FROM public.auction_status WHERE id = currval('items_id_seq')"
    expected = "Empty Auction"
  }
  assert_eq {
    query    = "SELECT approved_bid_count FROM public.auction_status WHERE id = currval('items_id_seq')"
    expected = "0"
  }
  assert = [
    "SELECT highest_bid IS NULL FROM public.auction_status WHERE id = currval('items_id_seq')",
    "SELECT effective_end = (SELECT end_at FROM public.items WHERE id = currval('items_id_seq')) FROM public.auction_status WHERE id = currval('items_id_seq')",
  ]
}

test "view_counts_only_approved_bids" {
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Vintage Watch', NOW() - interval '1 hour', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'bob', 200)",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'charlie', 50)",
  ]
  assert_eq {
    query    = "SELECT approved_bid_count FROM public.auction_status WHERE id = currval('items_id_seq')"
    expected = "2"
  }
  assert_eq {
    query    = "SELECT highest_bid FROM public.auction_status WHERE id = currval('items_id_seq')"
    expected = "200"
  }
}

test "view_effective_end_extends_with_bids" {
  # Bids placed at NOW(), so effective_end = GREATEST(end_at, NOW()+30s)
  # With end_at only 10 seconds from now, the bid extends it
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Ending Soon', NOW() - interval '1 hour', NOW() + interval '10 seconds')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
  ]
  assert = [
    "SELECT effective_end > (SELECT end_at FROM public.items WHERE id = currval('items_id_seq')) FROM public.auction_status WHERE id = currval('items_id_seq')",
    "SELECT effective_end = NOW() + interval '30 seconds' FROM public.auction_status WHERE id = currval('items_id_seq')",
  ]
}

test "view_effective_end_uses_end_at_when_no_recent_bids" {
  # end_at is 1 hour from now, bid was placed 1 hour ago
  # effective_end = GREATEST(NOW()+1h, NOW()-1h+30s) = NOW()+1h = end_at
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Long Auction', NOW() - interval '2 hours', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
    "UPDATE public.bids SET created_at = NOW() - interval '1 hour' WHERE item_id = currval('items_id_seq')",
  ]
  assert = [
    "SELECT effective_end = (SELECT end_at FROM public.items WHERE id = currval('items_id_seq')) FROM public.auction_status WHERE id = currval('items_id_seq')",
  ]
}

# --- Scenario Tests (requires --features scenario and clingo) ---
# Each answer set becomes a separate test that exercises the bid processing
# triggers with a different combination of bidders and amounts.

scenario "bid_processing" {
  program = <<-ASP
    bidder(alice; bob; charlie).
    amount(50; 100; 200; 500).

    % Generate 1-3 bids, each bidder bids at most once
    1 { bid(B, A) : bidder(B), amount(A) } 3.
    :- bid(B, A1), bid(B, A2), A1 != A2.
  ASP

  setup = [
    "INSERT INTO items (name, start_at, end_at) VALUES ('Test Item', NOW() - interval '1 hour', NOW() + interval '1 hour')",
  ]

  map "bid" {
    sql = "INSERT INTO bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), '{1}', {2})"
  }

  runs = 10
}

scenario "bid_ordering" {
  # Demonstrates: order_by (#5) and check (#6)
  # Bids are inserted in ascending amount order so the last insert
  # always holds the current highest bid.
  program = <<-ASP
    bidder(alice; bob; charlie).
    amount(50; 100; 200).

    % Each bidder places exactly one bid
    1 { bid(B, A) : amount(A) } 1 :- bidder(B).
    :- bid(B, A1), bid(B, A2), A1 != A2.
    % All three bidders must bid
    :- bidder(B), not 1 { bid(B, _) }.
    % All amounts must differ
    :- bid(B1, A), bid(B2, A), B1 != B2.
  ASP

  setup = [
    "INSERT INTO items (name, start_at, end_at) VALUES ('Ordered Item', NOW() - interval '1 hour', NOW() + interval '1 hour')",
  ]

  map "bid" {
    sql      = "INSERT INTO bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), '{1}', {2})"
    order_by = 2
  }

  check "highest_bid_is_approved" {
    assert = [
      "SELECT br.status = 'approved' FROM bid_results br JOIN bids b ON b.id = br.bid_id WHERE b.amount = (SELECT MAX(amount) FROM bids WHERE item_id = currval('items_id_seq'))",
    ]
  }

  runs = 6
}

scenario "invalid_bids" {
  # Demonstrates: expect_error (#7)
  # Generates bids with amount 0 which violates the positive_amount check
  # constraint. The test passes because we expect the SQL to fail.
  program = <<-ASP
    bidder(alice; bob).
    amount(0; 100).

    1 { bid(B, A) : bidder(B), amount(A) } 2.
    :- bid(B, A1), bid(B, A2), A1 != A2.
    % At least one bid must use the invalid amount
    :- not bid(_, 0).
  ASP

  setup = [
    "INSERT INTO items (name, start_at, end_at) VALUES ('Error Item', NOW() - interval '1 hour', NOW() + interval '1 hour')",
  ]

  map "bid" {
    sql = "INSERT INTO bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), '{1}', {2})"
  }

  expect_error = true
  runs = 5
}

scenario "bid_snapshot" {
  # Demonstrates: params (#10), assert_eq (#8), assert_snapshot (#8),
  #               teardown (#11)
  # Uses params to inject a #const limiting the ASP search space.
  program = <<-ASP
    bidder(alice; bob).
    amount(100; 200).

    % Each bidder bids once with a distinct amount
    1 { bid(B, A) : amount(A) } 1 :- bidder(B).
    :- bid(B, A1), bid(B, A2), A1 != A2.
    :- bidder(B), not 1 { bid(B, _) }.
    :- bid(B1, A), bid(B2, A), B1 != B2.
  ASP

  params = {
    max_bids = "2"
  }

  setup = [
    "INSERT INTO items (name, start_at, end_at) VALUES ('Snapshot Item', NOW() - interval '1 hour', NOW() + interval '1 hour')",
  ]

  map "bid" {
    sql      = "INSERT INTO bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), '{1}', {2})"
    order_by = 2
  }

  assert_eq {
    query    = "SELECT COUNT(*)::text FROM bid_results WHERE item_id = currval('items_id_seq') AND status = 'approved'"
    expected = "2"
  }

  assert_snapshot {
    query = "SELECT status::text FROM bid_results WHERE item_id = currval('items_id_seq') ORDER BY id"
    rows = [
      ["approved"],
      ["approved"],
    ]
  }

  teardown = [
    "DELETE FROM bid_results WHERE item_id IN (SELECT id FROM items WHERE name = 'Snapshot Item')",
    "DELETE FROM bids WHERE item_id IN (SELECT id FROM items WHERE name = 'Snapshot Item')",
    "DELETE FROM items WHERE name = 'Snapshot Item'",
  ]

  runs = 2
}

# --- Notify Tests ---
# These tests use the committed (non-transactional) path with teardown cleanup.

test "notify_on_approved_bid" {
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Notify Test', NOW() - interval '1 hour', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
  ]
  assert_notify {
    channel          = "bid_result"
    payload_contains = "approved"
  }
  teardown = [
    "DELETE FROM public.bid_results",
    "DELETE FROM public.bids",
    "DELETE FROM public.items",
  ]
}

test "notify_on_rejected_bid" {
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Notify Test', NOW() - interval '1 hour', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'bob', 50)",
  ]
  assert_notify {
    channel          = "bid_result"
    payload_contains = "rejected"
  }
  teardown = [
    "DELETE FROM public.bid_results",
    "DELETE FROM public.bids",
    "DELETE FROM public.items",
  ]
}

test "notify_payload_contains_bidder" {
  setup = [
    "INSERT INTO public.items (name, start_at, end_at) VALUES ('Notify Test', NOW() - interval '1 hour', NOW() + interval '1 hour')",
    "INSERT INTO public.bids (item_id, bidder, amount) VALUES (currval('items_id_seq'), 'alice', 100)",
  ]
  assert_notify {
    channel          = "bid_result"
    payload_contains = "alice"
  }
  teardown = [
    "DELETE FROM public.bid_results",
    "DELETE FROM public.bids",
    "DELETE FROM public.items",
  ]
}
