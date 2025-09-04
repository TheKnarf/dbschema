set shell := ["bash", "-cu"]

# Download the PGlite runtime artifacts via the crate's helper
pglite-assets:
  just --justfile crates/pglite/justfile pglite-assets

# Format HCL files in place
fmt *paths:
  cargo run -- fmt {{paths}}

###############################################
# Run examples against local Postgres (Docker)
###############################################

# Start Docker Postgres via compose and wait for readiness
docker-up:
  #!/usr/bin/env bash
  set -euo pipefail
  echo "[dbschema] Starting Postgres (Docker Compose)..."
  docker compose up -d
  echo "[dbschema] Waiting for Postgres to become ready..."
  for i in $(seq 1 60); do
    if docker compose exec -T db pg_isready -U postgres -d postgres >/dev/null 2>&1; then
      echo "[dbschema] Postgres is ready."
      exit 0
    fi
    sleep 1
  done
  echo "[dbschema] Postgres did not become ready in time" >&2
  exit 1

# Run end-to-end for a single example: create DB, apply migration, run tests, drop DB
# Usage:
#   just example-test file=examples/table.hcl
#   just example-test file=examples/trigger.hcl dsn=postgres://...

example-test file='examples/table.hcl' dsn-prefix='postgres://postgres:postgres@localhost:5432':
  #!/usr/bin/env bash
  set -euo pipefail
  name=$(basename "{{file}}" .hcl)
  db="dbschema_ex_${name}"
  dsn="{{dsn-prefix}}/${db}"
  GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; BLUE='\033[0;34m'; NC='\033[0m'
  echo -e "${BLUE}⏳ Running example: ${name}${NC}"
  # Ensure Docker is up
  just docker-up
  # Recreate DB
  docker compose exec -T db psql -U postgres -d postgres -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS \"${db}\";" -c "CREATE DATABASE \"${db}\";"
  # Apply migrations and run tests
  if cargo run -- --input "{{file}}" test --dsn "$dsn" --apply >/dev/null; then
    echo -e "${GREEN}✅ ${name} ok${NC}"
    rc=0
  else
    echo -e "${RED}❌ ${name} failed${NC}"
    rc=1
  fi
  # Drop DB
  docker compose exec -T db psql -U postgres -d postgres -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS \"${db}\";" >/dev/null || true
  exit $rc

# Run create-migration for all example HCL files
examples-create-migration:
  #!/usr/bin/env bash
  set -euo pipefail
  for example in examples/*.hcl; do
    name=$(basename "$example" .hcl)
    outdir="tmp_mig_${name}"
    rm -rf "$outdir"
    mkdir -p "$outdir"
    cargo run -- --input "$example" create-migration --out-dir "$outdir" --name "$name"
    rm -rf "$outdir"
  done

# Run tests for all example HCL files against local Postgres (Docker)
examples-test dsn-prefix='postgres://postgres:postgres@localhost:5432':
  #!/usr/bin/env bash
  set -euo pipefail
  just docker-up
  GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; BLUE='\033[0;34m'; NC='\033[0m'
  passed=0; failed=0
  for example in examples/*.hcl; do
    name=$(basename "$example" .hcl)
    db="dbschema_ex_${name}"
    dsn="{{dsn-prefix}}/${db}"
    echo -e "${BLUE}⏳ Running example: ${name}${NC}"
    # Recreate DB
    if ! docker compose exec -T db psql -U postgres -d postgres -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS \"${db}\";" -c "CREATE DATABASE \"${db}\";" >/dev/null; then
      echo -e "${RED}❌ ${name} failed (create db)${NC}"; failed=$((failed+1)); continue
    fi
    # Apply migrations and run tests
    if cargo run -- --input "$example" test --dsn "$dsn" --apply >/dev/null; then
      echo -e "${GREEN}✅ ${name} ok${NC}"; passed=$((passed+1))
    else
      echo -e "${RED}❌ ${name} failed (tests)${NC}"; failed=$((failed+1))
    fi
    # Drop DB
    docker compose exec -T db psql -U postgres -d postgres -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS \"${db}\";" >/dev/null || true
  done
  echo -e "${YELLOW}Summary: ${passed} passed, ${failed} failed${NC}"

# Validate all example HCL files
examples-validate:
  #!/usr/bin/env bash
  set -euo pipefail
  for example in examples/*.hcl; do
    cargo run -- --input "$example" validate
  done
