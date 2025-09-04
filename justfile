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

# Run tests for a single example file against a local Postgres started via Docker Compose
# Usage:
#   just example-test file=examples/table.hcl                 # uses default DSN
#   just example-test file=examples/trigger.hcl dsn=postgres://...

example-test file='examples/table.hcl' dsn='postgres://postgres:postgres@localhost:5432/dbschema_dev':
  set -euo pipefail
  echo "Starting Postgres with docker compose..."
  docker compose up -d
  echo "Waiting for Postgres to become ready..."
  for i in $(seq 1 60); do \
    if docker compose exec -T db pg_isready -U postgres -d dbschema_dev >/dev/null 2>&1; then \
      echo "Postgres is ready."; \
      break; \
    fi; \
    sleep 1; \
  done
  echo "Running tests for {{file}} via cargo..."
  cargo run -- --input "{{file}}" test --dsn "{{dsn}}"

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
examples-test dsn='postgres://postgres:postgres@localhost:5432/dbschema_dev':
  #!/usr/bin/env bash
  set -euo pipefail
  echo "Starting Postgres with docker compose..."
  docker compose up -d
  echo "Waiting for Postgres to become ready..."
  for i in $(seq 1 60); do
    if docker compose exec -T db pg_isready -U postgres -d dbschema_dev >/dev/null 2>&1; then
      echo "Postgres is ready."
      break
    fi
    sleep 1
  done
  for example in examples/*.hcl; do
    echo "Running tests: $example"
    cargo run -- --input "$example" test --dsn "{{dsn}}"
  done

# Validate all example HCL files
examples-validate:
  #!/usr/bin/env bash
  set -euo pipefail
  for example in examples/*.hcl; do
    cargo run -- --input "$example" validate
  done
