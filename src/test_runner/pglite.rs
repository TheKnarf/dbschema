use anyhow::{anyhow, Result};
use std::collections::HashSet;
use wasmtime::{Engine, Instance, Linker, Module, Store};
use wasmtime_wasi::{preview1::{add_to_linker_sync, WasiP1Ctx}, DirPerms, FilePerms, WasiCtxBuilder};

use super::{TestBackend, TestSummary};
use crate::ir::Config;

/// In-memory Postgres backend powered by the PGlite WASM build.
///
/// This backend bootstraps a WASI environment, instantiates the
/// pre-built `pglite.wasm` module and exposes a minimal wrapper
/// around the exported functions required to initialise, run and
/// shutdown the in-memory database. Query execution through the
/// Postgres wire protocol is not yet implemented.
pub struct PGliteRuntime {
    store: Store<WasiP1Ctx>,
    instance: Instance,
}

impl PGliteRuntime {
    /// Load the PGlite module and initialise the database files.
    fn new() -> Result<Self> {
        // Locate the bundled wasm and data files
        let wasm_path = "vendor/pglite/pglite.wasm";

        let engine = Engine::default();
        let module = Module::from_file(&engine, wasm_path)?;
        let mut linker = Linker::new(&engine);
        add_to_linker_sync(&mut linker, |ctx: &mut WasiP1Ctx| ctx)?;

        // Preopen the directory containing pglite.data as /tmp/pglite
        let mut builder = WasiCtxBuilder::new();
        builder
            .inherit_stdio()
            .preopened_dir(
                "vendor/pglite",
                "/tmp/pglite",
                DirPerms::all(),
                FilePerms::all(),
            )?;
        let wasi = builder.build_p1();
        let mut store = Store::new(&engine, wasi);
        let instance = linker.instantiate(&mut store, &module)?;

        // Call _pgl_initdb to ensure the database files are set up
        let init = instance.get_typed_func::<(), i32>(&mut store, "_pgl_initdb")?;
        let rc = init.call(&mut store, ())?;
        if rc != 0 {
            return Err(anyhow!("pglite initdb failed with code {rc}"));
        }
        Ok(Self { store, instance })
    }

    /// Start the backend. Currently this simply invokes the exported
    /// `_pgl_backend` symbol which spins up the internal Postgres
    /// server. Query execution is not yet wired up.
    fn backend(&mut self) -> Result<()> {
        let func = self
            .instance
            .get_typed_func::<(), ()>(&mut self.store, "_pgl_backend")?;
        func.call(&mut self.store, ())?;
        Ok(())
    }

    /// Shutdown the backend and flush the filesystem.
    fn shutdown(&mut self) -> Result<()> {
        let func = self
            .instance
            .get_typed_func::<(), ()>(&mut self.store, "_pgl_shutdown")?;
        func.call(&mut self.store, ())?;
        Ok(())
    }
}

pub struct PGliteTestBackend;

impl TestBackend for PGliteTestBackend {
    fn run(
        &self,
        _cfg: &Config,
        _dsn: &str,
        _only: Option<&HashSet<String>>,
    ) -> Result<TestSummary> {
        let mut rt = PGliteRuntime::new()?;
        rt.backend()?;
        // TODO: send wire protocol messages and evaluate tests
        rt.shutdown()?;
        Err(anyhow!("PGlite backend query execution not yet implemented"))
    }
}

