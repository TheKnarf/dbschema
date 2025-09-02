use anyhow::{anyhow, Result};
use bytes::BytesMut;
use fallible_iterator::FallibleIterator;
use postgres_protocol::message::{backend, frontend};
use wasmtime::{Engine, Instance, Linker, Memory, Module, Store, TypedFunc};
use wasmtime_wasi::{
    preview1::{add_to_linker_sync, WasiP1Ctx},
    DirPerms, FilePerms, WasiCtxBuilder,
};

/// In-memory Postgres backend powered by the PGlite WASM build.
///
/// This backend bootstraps a WASI environment, instantiates the
/// pre-built `pglite.wasm` module and exposes a minimal wrapper
/// around the exported functions required to initialise, run and
/// shutdown the in-memory database. Query execution through the
/// Postgres wire protocol is handled via an in-memory bridge.
pub struct PGliteRuntime {
    store: Store<WasiP1Ctx>,
    _instance: Instance,
    memory: Memory,
    interactive_write: TypedFunc<i32, ()>,
    interactive_read: TypedFunc<(), i32>,
    get_channel: TypedFunc<(), i32>,
    use_wire: TypedFunc<i32, ()>,
    backend: TypedFunc<(), ()>,
    shutdown_fn: TypedFunc<(), ()>,
}

impl PGliteRuntime {
    /// Load the PGlite module and initialise the database files.
    pub fn new() -> Result<Self> {
        // Locate the bundled wasm and data files
        let wasm_path = "vendor/pglite/pglite.wasm";

        let engine = Engine::default();
        let module = Module::from_file(&engine, wasm_path)?;
        let mut linker = Linker::new(&engine);
        // Minimal host env required by the PGlite module
        linker.func_wrap("env", "invoke_iii", |_a: i32, _b: i32, _c: i32| -> i32 {
            0
        })?;
        linker.func_wrap(
            "env",
            "invoke_viiii",
            |_a: i32, _b: i32, _c: i32, _d: i32, _e: i32| {},
        )?;
        linker.func_wrap("env", "invoke_vi", |_a: i32, _b: i32| {})?;
        linker.func_wrap("env", "invoke_v", |_a: i32| {})?;
        linker.func_wrap("env", "invoke_j", |_a: i32| -> i64 { 0 })?;
        add_to_linker_sync(&mut linker, |ctx: &mut WasiP1Ctx| ctx)?;

        // Preopen the directory containing pglite.data as /tmp/pglite
        let mut builder = WasiCtxBuilder::new();
        builder.inherit_stdio().preopened_dir(
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

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| anyhow!("missing memory export"))?;
        let interactive_write =
            instance.get_typed_func::<i32, ()>(&mut store, "_interactive_write")?;
        let interactive_read =
            instance.get_typed_func::<(), i32>(&mut store, "_interactive_read")?;
        let get_channel = instance.get_typed_func::<(), i32>(&mut store, "_get_channel")?;
        let use_wire = instance.get_typed_func::<i32, ()>(&mut store, "_use_wire")?;
        let backend = instance.get_typed_func::<(), ()>(&mut store, "_pgl_backend")?;
        let shutdown_fn = instance.get_typed_func::<(), ()>(&mut store, "_pgl_shutdown")?;

        Ok(Self {
            store,
            _instance: instance,
            memory,
            interactive_write,
            interactive_read,
            get_channel,
            use_wire,
            backend,
            shutdown_fn,
        })
    }
    /// Execute a single protocol message and return the backend response bytes.
    fn exec_protocol(&mut self, message: &[u8]) -> Result<Vec<u8>> {
        self.use_wire.call(&mut self.store, 1)?;
        self.interactive_write
            .call(&mut self.store, message.len() as i32)?;
        let mem = self.memory.data_mut(&mut self.store);
        mem[1..1 + message.len()].copy_from_slice(message);
        self.backend.call(&mut self.store, ())?;
        let chan = self.get_channel.call(&mut self.store, ())?;
        if chan <= 0 {
            return Err(anyhow!("unsupported channel"));
        }
        let out_len = self.interactive_read.call(&mut self.store, ())? as usize;
        let start = message.len() + 2;
        let end = start + out_len;
        let mem = self.memory.data(&self.store);
        Ok(mem[start..end].to_vec())
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
        self.shutdown_fn.call(&mut self.store, ())?;
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
