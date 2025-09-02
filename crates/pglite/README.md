# pglite

A Rust wrapper around the PGlite WASM build. This crate exposes a minimal
runtime for the [`@electric-sql/pglite` npm package](https://www.npmjs.com/package/@electric-sql/pglite),
allowing tests and applications to run an in-memory PostgreSQL instance via
WebAssembly.

## Setup

Download the required WebAssembly artifacts with:

```sh
just --justfile crates/pglite/justfile pglite-assets
# or from the workspace root
just pglite-assets
```

This fetches the `pglite.wasm` and `pglite.data` files into `vendor/pglite`.

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

