# pglite

A Rust wrapper around the PGlite WASM build. This crate exposes a minimal
runtime for the [`@electric-sql/pglite` npm package](https://www.npmjs.com/package/@electric-sql/pglite),
allowing tests and applications to run an in-memory PostgreSQL instance via
WebAssembly.

## Setup

Install the required WebAssembly package with:

```sh
just --justfile crates/pglite/justfile pglite-assets
# or from the workspace root
just pglite-assets
```

This runs `pnpm install`, placing `pglite.wasm` and `pglite.data` under
`node_modules/@electric-sql/pglite/dist` for the runtime to load.

## Usage

```rust
use pglite::PGliteRuntime;

fn main() -> anyhow::Result<()> {
    let mut rt = PGliteRuntime::new()?;
    rt.startup()?;
    let msgs = rt.simple_query("SELECT 1")?;
    rt.shutdown()?;
    Ok(())
}
```

The runtime exposes simple helpers for startup, query execution and shutdown.

