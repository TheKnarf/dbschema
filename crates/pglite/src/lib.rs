use anyhow::{anyhow, Result};
use bytes::BytesMut;
use fallible_iterator::FallibleIterator;
use postgres_protocol::message::{backend, frontend};
use std::collections::HashMap;
use std::path::PathBuf;
use wasmer::{FunctionEnv, Instance, Memory, Module, Store, Table, TypedFunction};
use wasmer_emscripten::{
    generate_emscripten_env, run_emscripten_instance, EmEnv, EmscriptenGlobals,
};
use wasmer_wasix::WasiEnv;

/// In-memory Postgres backend powered by the PGlite WASM build.
///
/// This backend bootstraps an Emscripten environment using Wasmer,
/// instantiates the pre-built `pglite.wasm` module and exposes a minimal
/// wrapper around the exported functions required to initialise, run and
/// shutdown the in-memory database. Query execution through the
/// Postgres wire protocol is handled via an in-memory bridge.
pub struct PGliteRuntime {
    store: Store,
    _instance: Instance,
    memory: Memory,
    interactive_write: TypedFunction<i32, ()>,
    interactive_read: TypedFunction<(), i32>,
    get_channel: TypedFunction<(), i32>,
    use_wire: TypedFunction<i32, ()>,
    backend: TypedFunction<(), ()>,
    shutdown_fn: TypedFunction<(), ()>,
    _table: Table,
}

impl PGliteRuntime {
    /// Initialise the PGlite runtime and underlying database.
    pub fn new() -> Result<Self> {
        let pkg_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("node_modules/@electric-sql/pglite/dist");
        let wasm_path = pkg_dir.join("pglite.wasm");
        let wasm_bytes = std::fs::read(&wasm_path)?;

        let mut store = Store::default();
        let module = Module::new(&store, wasm_bytes)?;

        let env = FunctionEnv::new(&mut store, EmEnv::new());
        let mut globals =
            EmscriptenGlobals::new(&mut store, &env, &module).map_err(|e| anyhow!(e))?;
        let mut mapped_dirs = HashMap::new();
        mapped_dirs.insert("/tmp/pglite".to_string(), pkg_dir.clone());
        mapped_dirs.insert(".".to_string(), pkg_dir.clone());
        env.as_ref(&store).set_data(&globals.data, mapped_dirs);

        let mut wasi_env = WasiEnv::builder("pglite").finalize(&mut store)?;

        let mut import_object = generate_emscripten_env(&mut store, &env, &mut globals);
        let wasi_imports = wasi_env.import_object(&mut store, &module)?;
        import_object.extend(&wasi_imports);
        let mut instance = Instance::new(&mut store, &module, &import_object)?;
        wasi_env.initialize(&mut store, instance.clone())?;

        // Set up memory, function pointers and run constructors. Ignore the
        // error about missing main/entrypoint as the module exposes its own API.
        let _ = run_emscripten_instance(
            &mut instance,
            env.clone().into_mut(&mut store),
            &mut globals,
            "",
            vec![],
            None,
        );

        // Call _pgl_initdb to ensure the database files are set up.
        let init = instance
            .exports
            .get_typed_function::<(), i32>(&mut store, "_pgl_initdb")?;
        let rc = init.call(&mut store)?;
        if rc != 0 {
            return Err(anyhow!("pglite initdb failed with code {rc}"));
        }

        let interactive_write = instance
            .exports
            .get_typed_function::<i32, ()>(&mut store, "_interactive_write")?;
        let interactive_read = instance
            .exports
            .get_typed_function::<(), i32>(&mut store, "_interactive_read")?;
        let get_channel = instance
            .exports
            .get_typed_function::<(), i32>(&mut store, "_get_channel")?;
        let use_wire = instance
            .exports
            .get_typed_function::<i32, ()>(&mut store, "_use_wire")?;
        let backend = instance
            .exports
            .get_typed_function::<(), ()>(&mut store, "_pgl_backend")?;
        let shutdown_fn = instance
            .exports
            .get_typed_function::<(), ()>(&mut store, "_pgl_shutdown")?;

        Ok(Self {
            store,
            _instance: instance,
            memory: globals.memory.clone(),
            interactive_write,
            interactive_read,
            get_channel,
            use_wire,
            backend,
            shutdown_fn,
            _table: globals.table.clone(),
        })
    }

    /// Execute a single protocol message and return the backend response bytes.
    fn exec_protocol(&mut self, message: &[u8]) -> Result<Vec<u8>> {
        self.use_wire.call(&mut self.store, 1)?;
        self.interactive_write
            .call(&mut self.store, message.len() as i32)?;
        {
            let view = self.memory.view(&self.store);
            view.write(1, message).map_err(|e| anyhow!(e.to_string()))?;
        }
        self.backend.call(&mut self.store)?;
        let chan = self.get_channel.call(&mut self.store)?;
        if chan <= 0 {
            return Err(anyhow!("unsupported channel"));
        }
        let out_len = self.interactive_read.call(&mut self.store)? as usize;
        let start = message.len() + 2;
        let mut out = vec![0u8; out_len];
        {
            let view = self.memory.view(&self.store);
            view.read(start as u64, &mut out)
                .map_err(|e| anyhow!(e.to_string()))?;
        }
        Ok(out)
    }

    /// Perform the initial startup handshake.
    pub fn startup(&mut self) -> Result<()> {
        let mut buf = BytesMut::new();
        let params = [("user", "postgres"), ("database", "postgres")];
        frontend::startup_message(params.iter().copied(), &mut buf)?;
        let resp = self.exec_protocol(&buf)?;
        let mut bytes = BytesMut::from(resp.as_slice());
        while let Some(msg) = backend::Message::parse(&mut bytes)? {
            if matches!(msg, backend::Message::ReadyForQuery(_)) {
                break;
            }
        }
        Ok(())
    }

    /// Execute a simple query and return backend messages.
    pub fn simple_query(&mut self, sql: &str) -> Result<Vec<backend::Message>> {
        let mut buf = BytesMut::new();
        frontend::query(sql, &mut buf)?;
        let resp = self.exec_protocol(&buf)?;
        let mut bytes = BytesMut::from(resp.as_slice());
        let mut messages = Vec::new();
        while let Some(msg) = backend::Message::parse(&mut bytes)? {
            if matches!(msg, backend::Message::ReadyForQuery(_)) {
                messages.push(msg);
                break;
            }
            messages.push(msg);
        }
        Ok(messages)
    }

    /// Shutdown the backend and flush the filesystem.
    pub fn shutdown(&mut self) -> Result<()> {
        self.shutdown_fn.call(&mut self.store)?;
        Ok(())
    }
}

/// Evaluate the first column of the first row for truthiness.
///
/// Supported types:
/// * text values "t" or "true" (case-insensitive)
/// * numeric strings, non-zero is treated as `true`
pub fn assert_row_true(row: &backend::DataRowBody) -> Result<bool> {
    let mut fields = row.ranges();
    if let Some(Some(range)) = fields.next()? {
        let buf = row.buffer();
        let val = &buf[range];
        if let Ok(s) = std::str::from_utf8(val) {
            if s.eq_ignore_ascii_case("t") || s.eq_ignore_ascii_case("true") {
                return Ok(true);
            }
            if let Ok(n) = s.parse::<i64>() {
                return Ok(n != 0);
            }
        }
    }
    Ok(false)
}
