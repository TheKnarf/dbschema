use anyhow::{anyhow, Result};
use bytes::BytesMut;
use fallible_iterator::FallibleIterator;
use postgres_protocol::message::{backend, frontend};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use wasmtime::FuncType;
use wasmtime::{
    Caller, Engine, ExternType, Global, Instance, Linker, Memory, Module, Ref, Store, Table,
    TypedFunc, Val, ValType,
};
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
    _table: Table,
}

fn valtype_from_char(c: char) -> ValType {
    match c {
        'i' => ValType::I32,
        'j' => ValType::I64,
        'f' => ValType::F32,
        'd' => ValType::F64,
        _ => ValType::I32,
    }
}

fn default_val(c: char) -> Val {
    match c {
        'i' => Val::I32(0),
        'j' => Val::I64(0),
        'f' => Val::F32(0.0f32.to_bits()),
        'd' => Val::F64(0.0f64.to_bits()),
        _ => Val::I32(0),
    }
}

fn default_valtype(t: ValType) -> Val {
    match t {
        ValType::I32 => Val::I32(0),
        ValType::I64 => Val::I64(0),
        ValType::F32 => Val::F32(0.0f32.to_bits()),
        ValType::F64 => Val::F64(0.0f64.to_bits()),
        _ => Val::I32(0),
    }
}

fn bind_module_imports(
    linker: &mut Linker<WasiP1Ctx>,
    store: &mut Store<WasiP1Ctx>,
    module: &Module,
    table_cell: Arc<Mutex<Option<Table>>>,
) -> Result<(Memory, Table)> {
    let mut memory: Option<Memory> = None;
    let mut table: Option<Table> = None;

    for import in module.imports() {
        let module_name = import.module();
        let name = import.name();
        match import.ty() {
            ExternType::Func(ty) if module_name == "env" => {
                if name.starts_with("invoke_") || name == "exit" || name == "getaddrinfo" {
                    continue;
                }
                let results: Vec<Val> = ty.results().map(default_valtype).collect();
                linker.func_new(module_name, name, ty.clone(), move |_c, _a, r| {
                    for (i, val) in results.iter().enumerate() {
                        r[i] = val.clone();
                    }
                    Ok(())
                })?;
            }
            ExternType::Memory(mt) if module_name == "env" && name == "memory" => {
                let mem = Memory::new(&mut *store, mt)?;
                linker.define(&mut *store, module_name, name, mem.clone())?;
                memory = Some(mem);
            }
            ExternType::Table(tt)
                if module_name == "env" && name == "__indirect_function_table" =>
            {
                let tbl = Table::new(&mut *store, tt, Ref::Func(None))?;
                linker.define(&mut *store, module_name, name, tbl.clone())?;
                *table_cell.lock().unwrap() = Some(tbl.clone());
                table = Some(tbl);
            }
            ExternType::Global(gt) if module_name == "env" => {
                let content = gt.content().clone();
                let g = Global::new(&mut *store, gt, default_valtype(content))?;
                linker.define(&mut *store, module_name, name, g)?;
            }
            ExternType::Global(gt) if module_name == "GOT.mem" && name == "__heap_base" => {
                let content = gt.content().clone();
                let g = Global::new(&mut *store, gt, default_valtype(content))?;
                linker.define(&mut *store, module_name, name, g)?;
            }
            _ => {}
        }
    }

    let memory = memory.ok_or_else(|| anyhow!("missing memory import"))?;
    let table = table.ok_or_else(|| anyhow!("missing table import"))?;
    Ok((memory, table))
}

fn bind_invoke_funcs(
    linker: &mut Linker<WasiP1Ctx>,
    table: Arc<Mutex<Option<Table>>>,
) -> Result<()> {
    fn bind(
        linker: &mut Linker<WasiP1Ctx>,
        table: Arc<Mutex<Option<Table>>>,
        sig: &str,
    ) -> Result<()> {
        let name = format!("invoke_{}", sig);
        let mut chars = sig.chars();
        let ret = chars.next().unwrap_or('v');
        let mut params: Vec<ValType> = vec![ValType::I32];
        for c in chars.clone() {
            params.push(valtype_from_char(c));
        }
        let results: Vec<ValType> = if ret == 'v' {
            vec![]
        } else {
            vec![valtype_from_char(ret)]
        };
        let ty = FuncType::new(linker.engine(), params.clone(), results.clone());
        let table_clone = table.clone();
        linker.func_new(
            "env",
            &name,
            ty,
            move |mut caller: Caller<'_, WasiP1Ctx>, args: &[Val], rets: &mut [Val]| {
                let index = args[0]
                    .i32()
                    .ok_or_else(|| anyhow!("invalid function index"))?;
                let func = {
                    let tbl_opt = table_clone.lock().unwrap();
                    let tbl = tbl_opt
                        .as_ref()
                        .ok_or_else(|| anyhow!("table not initialized"))?;
                    let val = tbl
                        .get(&mut caller, index as u64)
                        .ok_or_else(|| anyhow!("table lookup failed"))?;
                    match val {
                        Ref::Func(Some(f)) => f,
                        _ => return Err(anyhow!("expected funcref")),
                    }
                };
                let wasm_args = &args[1..];
                let mut wasm_rets = if rets.is_empty() {
                    vec![]
                } else {
                    vec![default_val(ret)]
                };
                func.call(&mut caller, wasm_args, &mut wasm_rets)?;
                if !rets.is_empty() {
                    rets[0] = wasm_rets[0].clone();
                }
                Ok(())
            },
        )?;
        Ok(())
    }

    for sig in [
        "di",
        "i",
        "id",
        "ii",
        "iii",
        "iiii",
        "iiiii",
        "iiiiii",
        "iiiiiii",
        "iiiiiiii",
        "iiiiiiiii",
        "iiiiiiiiii",
        "iiiiiiiiiii",
        "iiiiiiiiiiiiii",
        "iiiiiiiiiiiiiiiiii",
        "iiiiiji",
        "iiiij",
        "iiiijii",
        "iiij",
        "iiji",
        "ij",
        "ijiiiii",
        "ijiiiiii",
        "j",
        "ji",
        "jii",
        "jiiii",
        "jiiiiii",
        "jiiiiiiiii",
        "v",
        "vi",
        "vid",
        "vii",
        "viii",
        "viiii",
        "viiiii",
        "viiiiii",
        "viiiiiii",
        "viiiiiiii",
        "viiiiiiiii",
        "viiiiiiiiiiii",
        "viiiji",
        "viij",
        "viiji",
        "viijii",
        "viijiiii",
        "vij",
        "viji",
        "vijiji",
        "vj",
        "vji",
    ] {
        bind(linker, table.clone(), sig)?;
    }
    Ok(())
}
impl PGliteRuntime {
    /// Load the PGlite module and initialise the database files.
    pub fn new() -> Result<Self> {
        // Locate the bundled wasm and data files within node_modules
        let pkg_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("node_modules/@electric-sql/pglite/dist");
        let wasm_path = pkg_dir.join("pglite.wasm");

        let engine = Engine::default();
        let module = Module::from_file(&engine, &wasm_path)?;
        let mut linker = Linker::new(&engine);
        let table_cell: Arc<Mutex<Option<Table>>> = Arc::new(Mutex::new(None));
        bind_invoke_funcs(&mut linker, table_cell.clone())?;
        add_to_linker_sync(&mut linker, |ctx: &mut WasiP1Ctx| ctx)?;
        // PGlite expects an `env::exit` import; implement it via a host panic
        // to signal termination back to the caller.
        linker.func_wrap::<_, ()>("env", "exit", |_: Caller<'_, WasiP1Ctx>, code: i32| {
            panic!("env::exit({code})")
        })?;

        // PGlite may import `env::getaddrinfo` even though networking isn't
        // supported in this runtime. Provide a stub that always fails to
        // resolve addresses so module instantiation succeeds.
        linker.func_wrap(
            "env",
            "getaddrinfo",
            |_: Caller<'_, WasiP1Ctx>, _node: i32, _service: i32, _hints: i32, _res: i32| {
                // Return a non-zero error code (e.g. EAI_NONAME)
                1
            },
        )?;

        // Preopen the directory containing pglite.data as /tmp/pglite
        let mut builder = WasiCtxBuilder::new();
        builder.inherit_stdio().preopened_dir(
            &pkg_dir,
            "/tmp/pglite",
            DirPerms::all(),
            FilePerms::all(),
        )?;
        let wasi = builder.build_p1();
        let mut store = Store::new(&engine, wasi);

        // Provide memory, table, globals and any additional stub functions
        let (memory, table) =
            bind_module_imports(&mut linker, &mut store, &module, table_cell.clone())?;

        let instance = linker.instantiate(&mut store, &module)?;

        // Call _pgl_initdb to ensure the database files are set up
        let init = instance.get_typed_func::<(), i32>(&mut store, "_pgl_initdb")?;
        let rc = init.call(&mut store, ())?;
        if rc != 0 {
            return Err(anyhow!("pglite initdb failed with code {rc}"));
        }

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
            _table: table,
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
