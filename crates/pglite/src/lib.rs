use anyhow::{anyhow, Result};
use bytes::BytesMut;
use fallible_iterator::FallibleIterator;
use postgres_protocol::message::{backend, frontend};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use wasmer::{
    AsStoreMut, Exports, Function, FunctionEnv, FunctionEnvMut, Global, Imports, Instance, Memory,
    Module, Store, Table, TypedFunction, Value,
};
use wasmer_emscripten::{generate_emscripten_env, EmEnv, EmscriptenGlobals};

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
    get_buffer_size: Option<Function>,
    get_buffer_addr: Option<Function>,
    use_wire: TypedFunction<i32, ()>,
    backend: TypedFunction<(), ()>,
    shutdown_fn: TypedFunction<(), ()>,
    _table: Table,
}

impl PGliteRuntime {
    /// Initialise the PGlite runtime and underlying database.
    pub fn new() -> Result<Self> {
        eprintln!("[pglite] new(): begin");
        let pkg_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("node_modules/@electric-sql/pglite/dist");
        let wasm_path = pkg_dir.join("pglite.wasm");
        let wasm_bytes = std::fs::read(&wasm_path)?;
        eprintln!(
            "[pglite] read wasm {} ({} bytes)",
            wasm_path.display(),
            wasm_bytes.len()
        );

        // Use Store::default(); with wasmer singlepass feature enabled.
        let mut store = Store::default();
        let module = Module::new(&store, wasm_bytes)?;
        eprintln!("[pglite] module compiled");

        let env = FunctionEnv::new(&mut store, EmEnv::new());
        let mut globals =
            EmscriptenGlobals::new(&mut store, &env, &module).map_err(|e| anyhow!(e))?;
        eprintln!("[pglite] emscripten globals created");
        let mut mapped_dirs = HashMap::new();
        // Mount a writable temp directory for the database data files
        let host_tmp = std::env::temp_dir().join("pglite-db");
        let _ = std::fs::create_dir_all(&host_tmp);
        // Create a subset of expected directory layout under /tmp/pglite to satisfy init checks
        let subdirs = [
            "bin",
            "lib",
            "lib/postgresql",
            "lib/postgresql/pgxs",
            "lib/postgresql/pgxs/config",
            "lib/postgresql/pgxs/src",
            "lib/postgresql/pgxs/src/makefiles",
            "share",
            "share/postgresql",
            "share/postgresql/extension",
            "share/postgresql/timezone",
            "share/postgresql/timezone/Africa",
            "share/postgresql/timezone/America",
            "share/postgresql/timezone/America/Argentina",
            "share/postgresql/timezone/America/Indiana",
            "share/postgresql/timezone/America/Kentucky",
            "share/postgresql/timezone/America/North_Dakota",
            "share/postgresql/timezone/Antarctica",
            "share/postgresql/timezone/Arctic",
            "share/postgresql/timezone/Asia",
            "share/postgresql/timezone/Atlantic",
            "share/postgresql/timezone/Australia",
            "share/postgresql/timezone/Brazil",
            "share/postgresql/timezone/Canada",
            "share/postgresql/timezone/Chile",
            "share/postgresql/timezone/Etc",
            "share/postgresql/timezone/Europe",
        ];
        for d in &subdirs {
            let _ = std::fs::create_dir_all(host_tmp.join(d));
        }
        mapped_dirs.insert("/tmp/pglite".to_string(), host_tmp.clone());
        mapped_dirs.insert("/data".to_string(), host_tmp);
        // Mount the package dist for read-only assets
        mapped_dirs.insert("/pkg".to_string(), pkg_dir.clone());
        env.as_mut(&mut store).set_data(&globals.data, mapped_dirs);

        let mut import_object = generate_emscripten_env(&mut store, &env, &mut globals);
        eprintln!("[pglite] emscripten env generated");
        let t_ty = globals.table.ty(&store);
        eprintln!(
            "[pglite] base table min={} max={:?}",
            t_ty.minimum, t_ty.maximum
        );
        let m_ty = globals.memory.ty(&store);
        eprintln!(
            "[pglite] base memory min={} max={:?}",
            m_ty.minimum.0, m_ty.maximum
        );
        // Prepare a lightweight env to back some helper shims and WASI
        #[derive(Clone)]
        struct InvokeEnv {
            table: Table,
            memory: Memory,
            next_fd: i32,
            pipes: HashMap<i32, i32>, // fd -> peer fd (loopback pipe pair)
            pipe_bufs: HashMap<i32, VecDeque<u8>>, // readable buffers per fd
            sockets: HashMap<i32, VecDeque<u8>>, // simple loopback socket queues
        }
        let invoke_env = FunctionEnv::new(
            &mut store,
            InvokeEnv {
                table: globals.table.clone(),
                memory: globals.memory.clone(),
                next_fd: 3,
                pipes: HashMap::new(),
                pipe_bufs: HashMap::new(),
                sockets: HashMap::new(),
            },
        );

        // Early minimal WASI registration so imports exist
        {
            let mut wasi = Exports::new();
            let write_u32 = |mem: &Memory, store: &mut wasmer::StoreMut<'_>, ptr: u32, val: u32| {
                if ptr != 0 {
                    let view = mem.view(store);
                    let _ = view.write(ptr as u64, &val.to_le_bytes());
                }
            };
            wasi.insert(
                "environ_sizes_get",
                Function::new_typed_with_env(
                    &mut store,
                    &invoke_env,
                    move |mut env: FunctionEnvMut<InvokeEnv>, pcount: i32, psize: i32| -> i32 {
                        let mem = env.data().memory.clone();
                        let mut store_mut = env.as_store_mut();
                        write_u32(&mem, &mut store_mut, pcount as u32, 0);
                        write_u32(&mem, &mut store_mut, psize as u32, 0);
                        0
                    },
                ),
            );
            wasi.insert(
                "environ_get",
                Function::new_typed(&mut store, |_a: i32, _b: i32| -> i32 { 0 }),
            );
            wasi.insert(
                "proc_exit",
                Function::new_typed(&mut store, |_code: i32| {}),
            );
            wasi.insert(
                "fd_close",
                Function::new_typed(&mut store, |_a: i32| -> i32 { 0 }),
            );
            wasi.insert(
                "fd_sync",
                Function::new_typed(&mut store, |_a: i32| -> i32 { 0 }),
            );
            wasi.insert(
                "fd_fdstat_get",
                Function::new_typed(&mut store, |_a: i32, _b: i32| -> i32 { 0 }),
            );
            wasi.insert(
                "fd_seek",
                Function::new_typed_with_env(
                    &mut store,
                    &invoke_env,
                    move |mut env: FunctionEnvMut<InvokeEnv>,
                          _fd: i32,
                          _off: i64,
                          _wh: i32,
                          pout: i32|
                          -> i32 {
                        let mem = env.data().memory.clone();
                        let mut store_mut = env.as_store_mut();
                        write_u32(&mem, &mut store_mut, pout as u32, 0);
                        0
                    },
                ),
            );
            wasi.insert(
                "fd_read",
                Function::new_typed_with_env(
                    &mut store,
                    &invoke_env,
                    move |mut env: FunctionEnvMut<InvokeEnv>,
                          _fd: i32,
                          _iov: i32,
                          _ioc: i32,
                          nread: i32|
                          -> i32 {
                        let mem = env.data().memory.clone();
                        let mut store_mut = env.as_store_mut();
                        write_u32(&mem, &mut store_mut, nread as u32, 0);
                        0
                    },
                ),
            );
            wasi.insert(
                "fd_write",
                Function::new_typed_with_env(
                    &mut store,
                    &invoke_env,
                    move |mut env: FunctionEnvMut<InvokeEnv>,
                          _fd: i32,
                          _iov: i32,
                          _ioc: i32,
                          nw: i32|
                          -> i32 {
                        let mem = env.data().memory.clone();
                        let mut store_mut = env.as_store_mut();
                        write_u32(&mem, &mut store_mut, nw as u32, u32::MAX);
                        0
                    },
                ),
            );
            wasi.insert(
                "fd_pread",
                Function::new_typed_with_env(
                    &mut store,
                    &invoke_env,
                    move |mut env: FunctionEnvMut<InvokeEnv>,
                          _fd: i32,
                          _iov: i32,
                          _ioc: i32,
                          _off: i64,
                          nread: i32|
                          -> i32 {
                        let mem = env.data().memory.clone();
                        let mut store_mut = env.as_store_mut();
                        write_u32(&mem, &mut store_mut, nread as u32, 0);
                        0
                    },
                ),
            );
            wasi.insert(
                "fd_pwrite",
                Function::new_typed_with_env(
                    &mut store,
                    &invoke_env,
                    move |mut env: FunctionEnvMut<InvokeEnv>,
                          _fd: i32,
                          _iov: i32,
                          _ioc: i32,
                          _off: i64,
                          nw: i32|
                          -> i32 {
                        let mem = env.data().memory.clone();
                        let mut store_mut = env.as_store_mut();
                        write_u32(&mem, &mut store_mut, nw as u32, u32::MAX);
                        0
                    },
                ),
            );
            wasi.insert(
                "clock_time_get",
                Function::new_typed_with_env(
                    &mut store,
                    &invoke_env,
                    move |mut env: FunctionEnvMut<InvokeEnv>,
                          _clock: i32,
                          _prec: i64,
                          tp: i32|
                          -> i32 {
                        if tp != 0 {
                            let mem = env.data().memory.clone();
                            let store_mut = env.as_store_mut();
                            let view = mem.view(&store_mut);
                            let _ = view.write(tp as u64, &0u64.to_le_bytes());
                        }
                        0
                    },
                ),
            );
            wasi.insert(
                "random_get",
                Function::new_typed_with_env(
                    &mut store,
                    &invoke_env,
                    move |mut env: FunctionEnvMut<InvokeEnv>, buf: i32, len: i32| -> i32 {
                        if len > 0 && buf != 0 {
                            let mem = env.data().memory.clone();
                            let store_mut = env.as_store_mut();
                            let view = mem.view(&store_mut);
                            let tmp = vec![0u8; len as usize];
                            let _ = view.write(buf as u64, &tmp);
                        }
                        0
                    },
                ),
            );
            import_object.register_namespace("wasi_snapshot_preview1", wasi);
            eprintln!("[pglite] early-registered wasi_snapshot_preview1 imports");
        }

        // WASI shims are also registered later below alongside other env shims

        // Shim missing Emscripten invoke thunk expected by the module.
        // Provide env.invoke_vji: (i32 func_idx, i64, i32) -> () that
        // performs an indirect call through the module's function table.
        // This approximates Emscripten's invoke_* helpers.
        // (invoke_env already created)
        let mut env_ns = Exports::new();
        // Important: do not override Emscripten-provided memory or table.
        // We only provide a mutable `__stack_pointer` Global if the base env
        // doesn’t expose one (Wasmer `extend` will keep the existing symbol
        // when present in the base `import_object`).
        // Provide a mutable stack pointer global expected by Emscripten if missing.
        // Using a default 0; the module initializes it at runtime.
        let sp = Global::new_mut(&mut store, Value::I32(0));
        env_ns.insert("__stack_pointer", sp);
        // Provide the imported function table with the minimum size required
        // by the module, if present. We derive the min from the module's
        // declared import type to avoid size mismatches.
        // Register the Emscripten function table under the expected name.
        env_ns.insert("__indirect_function_table", globals.table.clone());
        let invoke_vji = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32, a1: i64, a2: i32| {
                if fidx < 0 {
                    return;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(i64, i32), ()>(&store_mut) {
                        let _ = typed.call(&mut store_mut, a1, a2);
                    }
                }
            },
        );
        env_ns.insert("invoke_vji", invoke_vji);
        // Emscripten inline asm const shim: return 0 by default.
        let asm_const_int =
            Function::new_typed(&mut store, |_code: i32, _sig: i32, _argv: i32| -> i32 { 0 });
        env_ns.insert("emscripten_asm_const_int", asm_const_int);
        // Force-exit shim: ignore requested exit code.
        let force_exit = Function::new_typed(&mut store, |_code: i32| {});
        env_ns.insert("emscripten_force_exit", force_exit);
        // Additional invoke thunk returning i64 with nine i32 args.
        let invoke_jiiiiiiiii = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i32,
             a2: i32,
             a3: i32,
             a4: i32,
             a5: i32,
             a6: i32,
             a7: i32,
             a8: i32,
             a9: i32|
             -> i64 {
                if fidx < 0 {
                    return 0;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) =
                        func.typed::<(i32, i32, i32, i32, i32, i32, i32, i32, i32), i64>(&store_mut)
                    {
                        if let Ok(ret) =
                            typed.call(&mut store_mut, a1, a2, a3, a4, a5, a6, a7, a8, a9)
                        {
                            return ret;
                        }
                    }
                }
                0
            },
        );
        env_ns.insert("invoke_jiiiiiiiii", invoke_jiiiiiiiii);
        // Additional invoke thunk returning i64 with six i32 args.
        let invoke_jiiiiii = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i32,
             a2: i32,
             a3: i32,
             a4: i32,
             a5: i32,
             a6: i32|
             -> i64 {
                if fidx < 0 {
                    return 0;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(i32, i32, i32, i32, i32, i32), i64>(&store_mut)
                    {
                        if let Ok(ret) = typed.call(&mut store_mut, a1, a2, a3, a4, a5, a6) {
                            return ret;
                        }
                    }
                }
                0
            },
        );
        env_ns.insert("invoke_jiiiiii", invoke_jiiiiii);
        // invoke_iiiiiiiiiiiiii: returns i32, takes 13 i32 args (plus func index)
        let invoke_iiiiiiiiiiiiii = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i32,
             a2: i32,
             a3: i32,
             a4: i32,
             a5: i32,
             a6: i32,
             a7: i32,
             a8: i32,
             a9: i32,
             a10: i32,
             a11: i32,
             a12: i32,
             a13: i32|
             -> i32 {
                if fidx < 0 {
                    return 0;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                    ), i32>(&store_mut)
                    {
                        if let Ok(ret) = typed.call(
                            &mut store_mut,
                            a1,
                            a2,
                            a3,
                            a4,
                            a5,
                            a6,
                            a7,
                            a8,
                            a9,
                            a10,
                            a11,
                            a12,
                            a13,
                        ) {
                            return ret;
                        }
                    }
                }
                0
            },
        );
        env_ns.insert("invoke_iiiiiiiiiiiiii", invoke_iiiiiiiiiiiiii.clone());
        // invoke_iiiijii: returns i32, args: i32, i32, i32, i64, i32, i32
        let invoke_iiiijii = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i32,
             a2: i32,
             a3: i32,
             a4: i64,
             a5: i32,
             a6: i32|
             -> i32 {
                if fidx < 0 {
                    return 0;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(i32, i32, i32, i64, i32, i32), i32>(&store_mut)
                    {
                        if let Ok(ret) = typed.call(&mut store_mut, a1, a2, a3, a4, a5, a6) {
                            return ret;
                        }
                    }
                }
                0
            },
        );
        env_ns.insert("invoke_iiiijii", invoke_iiiijii.clone());
        // invoke_vijiji: returns void, args: i32, i64, i32, i64, i32
        let invoke_vijiji = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i32,
             a2: i64,
             a3: i32,
             a4: i64,
             a5: i32| {
                if fidx < 0 {
                    return;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(i32, i64, i32, i64, i32), ()>(&store_mut) {
                        let _ = typed.call(&mut store_mut, a1, a2, a3, a4, a5);
                    }
                }
            },
        );
        env_ns.insert("invoke_vijiji", invoke_vijiji.clone());
        // invoke_iiiiiiiiiiiiiiiiii: return i32, 17 x i32 args
        let invoke_iiiiiiiiiiiiiiiiii = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i32,
             a2: i32,
             a3: i32,
             a4: i32,
             a5: i32,
             a6: i32,
             a7: i32,
             a8: i32,
             a9: i32,
             a10: i32,
             a11: i32,
             a12: i32,
             a13: i32,
             a14: i32,
             a15: i32,
             a16: i32,
             a17: i32|
             -> i32 {
                if fidx < 0 {
                    return 0;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                    ), i32>(&store_mut)
                    {
                        if let Ok(ret) = typed.call(
                            &mut store_mut,
                            a1,
                            a2,
                            a3,
                            a4,
                            a5,
                            a6,
                            a7,
                            a8,
                            a9,
                            a10,
                            a11,
                            a12,
                            a13,
                            a14,
                            a15,
                            a16,
                            a17,
                        ) {
                            return ret;
                        }
                    }
                }
                0
            },
        );
        env_ns.insert(
            "invoke_iiiiiiiiiiiiiiiiii",
            invoke_iiiiiiiiiiiiiiiiii.clone(),
        );
        // invoke_iiiij: return i32, args: i32, i32, i32, i64
        let invoke_iiiij = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i32,
             a2: i32,
             a3: i32,
             a4: i64|
             -> i32 {
                if fidx < 0 {
                    return 0;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(i32, i32, i32, i64), i32>(&store_mut) {
                        if let Ok(ret) = typed.call(&mut store_mut, a1, a2, a3, a4) {
                            return ret;
                        }
                    }
                }
                0
            },
        );
        env_ns.insert("invoke_iiiij", invoke_iiiij.clone());
        // invoke_viiiji: return void, args: i32, i32, i32, i64, i32
        let invoke_viiiji = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i32,
             a2: i32,
             a3: i32,
             a4: i64,
             a5: i32| {
                if fidx < 0 {
                    return;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(i32, i32, i32, i64, i32), ()>(&store_mut) {
                        let _ = typed.call(&mut store_mut, a1, a2, a3, a4, a5);
                    }
                }
            },
        );
        env_ns.insert("invoke_viiiji", invoke_viiiji.clone());
        // invoke_iiij: return i32, args: i32, i64
        let invoke_iiij = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32, a1: i32, a2: i32, a3: i64| -> i32 {
                if fidx < 0 {
                    return 0;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(i32, i32, i64), i32>(&store_mut) {
                        if let Ok(ret) = typed.call(&mut store_mut, a1, a2, a3) {
                            return ret;
                        }
                    }
                }
                0
            },
        );
        env_ns.insert("invoke_iiij", invoke_iiij.clone());
        // invoke_vid: return void, args: i32, f64
        let invoke_vid = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32, a1: i32, a2: f64| {
                if fidx < 0 {
                    return;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(i32, f64), ()>(&store_mut) {
                        let _ = typed.call(&mut store_mut, a1, a2);
                    }
                }
            },
        );
        env_ns.insert("invoke_vid", invoke_vid.clone());
        // invoke_ijiiiiii: return i32, args: i64, i32, i32, i32, i32, i32, i32
        let invoke_ijiiiiii = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i64,
             a2: i32,
             a3: i32,
             a4: i32,
             a5: i32,
             a6: i32,
             a7: i32|
             -> i32 {
                if fidx < 0 {
                    return 0;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) =
                        func.typed::<(i64, i32, i32, i32, i32, i32, i32), i32>(&store_mut)
                    {
                        if let Ok(ret) = typed.call(&mut store_mut, a1, a2, a3, a4, a5, a6, a7) {
                            return ret;
                        }
                    }
                }
                0
            },
        );
        env_ns.insert("invoke_ijiiiiii", invoke_ijiiiiii.clone());
        // invoke_viijii: return void, args: i32, i32, i64, i32, i32
        let invoke_viijii = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i32,
             a2: i32,
             a3: i64,
             a4: i32,
             a5: i32| {
                if fidx < 0 {
                    return;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(i32, i32, i64, i32, i32), ()>(&store_mut) {
                        let _ = typed.call(&mut store_mut, a1, a2, a3, a4, a5);
                    }
                }
            },
        );
        env_ns.insert("invoke_viijii", invoke_viijii.clone());
        // invoke_iiiiiji: return i32, args: i32, i32, i32, i32, i64, i32
        let invoke_iiiiiji = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i32,
             a2: i32,
             a3: i32,
             a4: i32,
             a5: i64,
             a6: i32|
             -> i32 {
                if fidx < 0 {
                    return 0;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(i32, i32, i32, i32, i64, i32), i32>(&store_mut)
                    {
                        if let Ok(ret) = typed.call(&mut store_mut, a1, a2, a3, a4, a5, a6) {
                            return ret;
                        }
                    }
                }
                0
            },
        );
        env_ns.insert("invoke_iiiiiji", invoke_iiiiiji.clone());
        // invoke_viijiiii: return void, args: i32, i32, i64, i32, i32, i32, i32
        let invoke_viijiiii = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i32,
             a2: i32,
             a3: i64,
             a4: i32,
             a5: i32,
             a6: i32,
             a7: i32| {
                if fidx < 0 {
                    return;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) =
                        func.typed::<(i32, i32, i64, i32, i32, i32, i32), ()>(&store_mut)
                    {
                        let _ = typed.call(&mut store_mut, a1, a2, a3, a4, a5, a6, a7);
                    }
                }
            },
        );
        env_ns.insert("invoke_viijiiii", invoke_viijiiii.clone());
        // invoke_jiiii: return i64, args: i32, i32, i32, i32
        let invoke_jiiii = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i32,
             a2: i32,
             a3: i32,
             a4: i32|
             -> i64 {
                if fidx < 0 {
                    return 0;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(i32, i32, i32, i32), i64>(&store_mut) {
                        if let Ok(ret) = typed.call(&mut store_mut, a1, a2, a3, a4) {
                            return ret;
                        }
                    }
                }
                0
            },
        );
        env_ns.insert("invoke_jiiii", invoke_jiiii.clone());
        // invoke_viiiiiiiiiiii: return void, args: 12 x i32
        let invoke_viiiiiiiiiiii = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i32,
             a2: i32,
             a3: i32,
             a4: i32,
             a5: i32,
             a6: i32,
             a7: i32,
             a8: i32,
             a9: i32,
             a10: i32,
             a11: i32,
             a12: i32| {
                if fidx < 0 {
                    return;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                        i32,
                    ), ()>(&store_mut)
                    {
                        let _ = typed.call(
                            &mut store_mut,
                            a1,
                            a2,
                            a3,
                            a4,
                            a5,
                            a6,
                            a7,
                            a8,
                            a9,
                            a10,
                            a11,
                            a12,
                        );
                    }
                }
            },
        );
        env_ns.insert("invoke_viiiiiiiiiiii", invoke_viiiiiiiiiiii.clone());
        // invoke_di: return f64, args: i32
        let invoke_di = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32, a1: i32| -> f64 {
                if fidx < 0 {
                    return 0.0;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<i32, f64>(&store_mut) {
                        if let Ok(ret) = typed.call(&mut store_mut, a1) {
                            return ret;
                        }
                    }
                }
                0.0
            },
        );
        env_ns.insert("invoke_di", invoke_di.clone());
        // invoke_id: return i32, args: f64
        let invoke_id = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32, a1: f64| -> i32 {
                if fidx < 0 {
                    return 0;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<f64, i32>(&store_mut) {
                        if let Ok(ret) = typed.call(&mut store_mut, a1) {
                            return ret;
                        }
                    }
                }
                0
            },
        );
        env_ns.insert("invoke_id", invoke_id.clone());
        // invoke_ijiiiii: return i32, args: i64, i32, i32, i32, i32, i32
        let invoke_ijiiiii = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>,
             fidx: i32,
             a1: i64,
             a2: i32,
             a3: i32,
             a4: i32,
             a5: i32,
             a6: i32|
             -> i32 {
                if fidx < 0 {
                    return 0;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<(i64, i32, i32, i32, i32, i32), i32>(&store_mut)
                    {
                        if let Ok(ret) = typed.call(&mut store_mut, a1, a2, a3, a4, a5, a6) {
                            return ret;
                        }
                    }
                }
                0
            },
        );
        env_ns.insert("invoke_ijiiiii", invoke_ijiiiii.clone());
        // Minimal syscall stubs expected by emscripten
        let syscall_fcntl64 =
            Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32| -> i32 { 0 });
        {
            env_ns.insert("__syscall_fcntl64", syscall_fcntl64.clone());
            let syscall_ioctl =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32| -> i32 { 0 });
            env_ns.insert("__syscall_ioctl", syscall_ioctl.clone());
            let syscall_openat =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32, _d: i32| -> i32 {
                    0
                });
            env_ns.insert("__syscall_openat", syscall_openat.clone());
            let tzset_js = Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32, _d: i32| {});
            env_ns.insert("_tzset_js", tzset_js.clone());
            let abort_js = Function::new_typed(&mut store, || {});
            env_ns.insert("_abort_js", abort_js.clone());
            let syscall_faccessat =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32, _d: i32| -> i32 {
                    0
                });
            env_ns.insert("__syscall_faccessat", syscall_faccessat.clone());
            let syscall_chdir = Function::new_typed(&mut store, |_a: i32| -> i32 { 0 });
            env_ns.insert("__syscall_chdir", syscall_chdir.clone());
            let syscall_chmod = Function::new_typed(&mut store, |_a: i32, _b: i32| -> i32 { 0 });
            env_ns.insert("__syscall_chmod", syscall_chmod.clone());
            let syscall_dup = Function::new_typed(&mut store, |a: i32| -> i32 { a });
            env_ns.insert("__syscall_dup", syscall_dup.clone());
            let syscall_dup3 =
                Function::new_typed(&mut store, |a: i32, _b: i32, _c: i32| -> i32 { a });
            env_ns.insert("__syscall_dup3", syscall_dup3.clone());
            let dlopen_js = Function::new_typed(&mut store, |_a: i32| -> i32 { 0 });
            env_ns.insert("_dlopen_js", dlopen_js.clone());
            let dlsym_js =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32| -> i32 { 0 });
            env_ns.insert("_dlsym_js", dlsym_js.clone());
            // Implement memcpy using the module memory (defensive bounds checks)
            let memcpy_js = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, dest: i32, src: i32, n: i32| {
                    if n <= 0 {
                        return;
                    }
                    if dest <= 0 || src <= 0 {
                        return;
                    }
                    let len = n as usize;
                    let memory = env.data().memory.clone();
                    let store = env.as_store_mut();
                    let view = memory.view(&store);
                    let mem_len = view.data_size();
                    let src_off = src as u64;
                    let dst_off = dest as u64;
                    if src_off + len as u64 > mem_len as u64 {
                        return;
                    }
                    if dst_off + len as u64 > mem_len as u64 {
                        return;
                    }
                    if len <= 4096 {
                        let mut tmp = [0u8; 4096];
                        let _ = view.read(src_off, &mut tmp[..len]);
                        let _ = view.write(dst_off, &tmp[..len]);
                    } else {
                        let mut buf = vec![0u8; len];
                        let _ = view.read(src_off, &mut buf);
                        let _ = view.write(dst_off, &buf);
                    }
                },
            );
            env_ns.insert("_emscripten_memcpy_js", memcpy_js.clone());
            let munmap_js = Function::new_typed(
                &mut store,
                |_a: i32, _b: i32, _c: i32, _d: i32, _e: i32, _f: i64| -> i32 { 0 },
            );
            env_ns.insert("_munmap_js", munmap_js.clone());
            let mmap_js = Function::new_typed(
                &mut store,
                |_a: i32, _b: i32, _c: i32, _d: i32, _e: i64, _f: i32, _g: i32| -> i32 { 0 },
            );
            env_ns.insert("_mmap_js", mmap_js.clone());
            let date_now = Function::new_typed(&mut store, || -> f64 {
                use std::time::{SystemTime, UNIX_EPOCH};
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default();
                now.as_secs_f64() * 1000.0
            });
            env_ns.insert("emscripten_date_now", date_now);
            let syscall_fdatasync = Function::new_typed(&mut store, |_a: i32| -> i32 { 0 });
            env_ns.insert("__syscall_fdatasync", syscall_fdatasync);
            let syscall_fstat64 = Function::new_typed(&mut store, |_a: i32, _b: i32| -> i32 { 0 });
            env_ns.insert("__syscall_fstat64", syscall_fstat64);
            let syscall_stat64 = Function::new_typed(&mut store, |_a: i32, _b: i32| -> i32 { 0 });
            env_ns.insert("__syscall_stat64", syscall_stat64);
            let syscall_newfstatat =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32, _d: i32| -> i32 {
                    0
                });
            env_ns.insert("__syscall_newfstatat", syscall_newfstatat);
            let syscall_lstat64 = Function::new_typed(&mut store, |_a: i32, _b: i32| -> i32 { 0 });
            env_ns.insert("__syscall_lstat64", syscall_lstat64);
            let syscall_ftruncate64 =
                Function::new_typed(&mut store, |_a: i32, _b: i64| -> i32 { 0 });
            env_ns.insert("__syscall_ftruncate64", syscall_ftruncate64);
            let syscall_getcwd = Function::new_typed(&mut store, |_a: i32, _b: i32| -> i32 { 0 });
            env_ns.insert("__syscall_getcwd", syscall_getcwd);
            let syscall_getdents64 =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32| -> i32 { 0 });
            env_ns.insert("__syscall_getdents64", syscall_getdents64);
            let get_now = Function::new_typed(&mut store, || -> f64 {
                use std::time::{SystemTime, UNIX_EPOCH};
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default();
                now.as_secs_f64() * 1000.0
            });
            env_ns.insert("emscripten_get_now", get_now);
            let em_lookup_name = Function::new_typed(&mut store, |_a: i32| -> i32 { 0 });
            env_ns.insert("_emscripten_lookup_name", em_lookup_name);
            let syscall_mkdirat =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32| -> i32 { 0 });
            env_ns.insert("__syscall_mkdirat", syscall_mkdirat);
            let localtime_js = Function::new_typed(&mut store, |_a: i64, _b: i32| {});
            env_ns.insert("_localtime_js", localtime_js.clone());
            let gmtime_js = Function::new_typed(&mut store, |_a: i64, _b: i32| {});
            env_ns.insert("_gmtime_js", gmtime_js.clone());
            let syscall_pipe = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, ptr: i32| -> i32 {
                    // allocate two new fds
                    let a;
                    let b;
                    {
                        a = env.data().next_fd;
                        env.data_mut().next_fd += 1;
                        b = env.data().next_fd;
                        env.data_mut().next_fd += 1;
                        env.data_mut().pipes.insert(a, b);
                        env.data_mut().pipes.insert(b, a);
                        env.data_mut()
                            .pipe_bufs
                            .entry(a)
                            .or_insert_with(VecDeque::new);
                        env.data_mut()
                            .pipe_bufs
                            .entry(b)
                            .or_insert_with(VecDeque::new);
                    }
                    let memory = env.data().memory.clone();
                    let store_mut = env.as_store_mut();
                    let view = memory.view(&store_mut);
                    let mut buf = [0u8; 8];
                    buf[..4].copy_from_slice(&a.to_le_bytes());
                    buf[4..].copy_from_slice(&b.to_le_bytes());
                    let mem_len = view.data_size() as u64;
                    let off = ptr as u64;
                    if off + 8 <= mem_len {
                        let _ = view.write(off, &buf);
                    }
                    0
                },
            );
            env_ns.insert("__syscall_pipe", syscall_pipe.clone());
            // read(fd, buf, count)
            let syscall_read = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, fd: i32, buf_ptr: i32, count: i32| -> i32 {
                    if count <= 0 {
                        return 0;
                    }
                    let len = count as usize;
                    let mut out: Vec<u8> = Vec::new();
                    let mut n = 0usize;
                    if let Some(q) = env.data_mut().pipe_bufs.get_mut(&fd) {
                        out.resize(len, 0);
                        while n < len {
                            if let Some(b) = q.pop_front() {
                                out[n] = b;
                                n += 1;
                            } else {
                                break;
                            }
                        }
                    } else if let Some(q) = env.data_mut().sockets.get_mut(&fd) {
                        out.resize(len, 0);
                        while n < len {
                            if let Some(b) = q.pop_front() {
                                out[n] = b;
                                n += 1;
                            } else {
                                break;
                            }
                        }
                    }
                    if n > 0 {
                        let memory = env.data().memory.clone();
                        let store_mut = env.as_store_mut();
                        let view = memory.view(&store_mut);
                        let mem_len = view.data_size() as u64;
                        let off = buf_ptr as u64;
                        let mut nn = n as u64;
                        if off >= mem_len {
                            nn = 0;
                        } else if off + nn > mem_len {
                            nn = mem_len - off;
                        }
                        if nn > 0 {
                            let _ = view.write(off, &out[..(nn as usize)]);
                            return nn as i32;
                        }
                        return 0;
                    }
                    // stdout/stderr: no-op
                    0
                },
            );
            env_ns.insert("__syscall_read", syscall_read.clone());
            // write(fd, buf, count)
            let syscall_write = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, fd: i32, buf_ptr: i32, count: i32| -> i32 {
                    if count <= 0 {
                        return 0;
                    }
                    let len = count as usize;
                    let mut tmp = vec![0u8; len];
                    {
                        let memory = env.data().memory.clone();
                        let store_mut = env.as_store_mut();
                        let view = memory.view(&store_mut);
                        let mem_len = view.data_size() as u64;
                        let off = buf_ptr as u64;
                        let mut nn = len as u64;
                        if off >= mem_len {
                            nn = 0;
                        } else if off + nn > mem_len {
                            nn = mem_len - off;
                        }
                        if nn > 0 {
                            let _ = view.read(off, &mut tmp[..(nn as usize)]);
                        }
                    }
                    // pipe write: push to peer queue
                    if let Some(&peer) = env.data().pipes.get(&fd) {
                        if let Some(q) = env.data_mut().pipe_bufs.get_mut(&peer) {
                            for b in &tmp {
                                q.push_back(*b);
                            }
                            return count;
                        }
                    }
                    // socket write: push back into the same socket queue (loopback)
                    if let Some(q) = env.data_mut().sockets.get_mut(&fd) {
                        for b in &tmp {
                            q.push_back(*b);
                        }
                        return count;
                    }
                    // stdout/stderr: drop
                    count
                },
            );
            env_ns.insert("__syscall_write", syscall_write.clone());
            let syscall_close = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, fd: i32| -> i32 {
                    env.data_mut().pipes.remove(&fd);
                    env.data_mut().pipe_bufs.remove(&fd);
                    env.data_mut().sockets.remove(&fd);
                    0
                },
            );
            env_ns.insert("__syscall_close", syscall_close.clone());
            // socket(domain, type, protocol)
            let syscall_socket = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>,
                 _a: i32,
                 _b: i32,
                 _c: i32,
                 _d: i32,
                 _e: i32,
                 _f: i32|
                 -> i32 {
                    let fd = env.data().next_fd;
                    env.data_mut().next_fd += 1;
                    env.data_mut()
                        .sockets
                        .entry(fd)
                        .or_insert_with(VecDeque::new);
                    fd
                },
            );
            env_ns.insert("__syscall_socket", syscall_socket.clone());
            let syscall_connect = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut _env: FunctionEnvMut<InvokeEnv>,
                 _fd: i32,
                 _a: i32,
                 _b: i32,
                 _c: i32,
                 _d: i32,
                 _e: i32|
                 -> i32 { 0 },
            );
            env_ns.insert("__syscall_connect", syscall_connect.clone());
            let syscall_listen = Function::new_typed(
                &mut store,
                |_a: i32, _b: i32, _c: i32, _d: i32, _e: i32, _f: i32| -> i32 { 0 },
            );
            env_ns.insert("__syscall_listen", syscall_listen.clone());
            let syscall_accept4 = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, _fd: i32, _a: i32, _b: i32, _c: i32| -> i32 {
                    let fd = env.data().next_fd;
                    env.data_mut().next_fd += 1;
                    env.data_mut()
                        .sockets
                        .entry(fd)
                        .or_insert_with(VecDeque::new);
                    fd
                },
            );
            env_ns.insert("__syscall_accept4", syscall_accept4.clone());
            let syscall_getsockname = Function::new_typed(
                &mut store,
                |_a: i32, _b: i32, _c: i32, _d: i32, _e: i32, _f: i32| -> i32 { 0 },
            );
            env_ns.insert("__syscall_getsockname", syscall_getsockname.clone());
            let syscall_getsockopt = Function::new_typed(
                &mut store,
                |_a: i32, _b: i32, _c: i32, _d: i32, _e: i32, _f: i32| -> i32 { 0 },
            );
            env_ns.insert("__syscall_getsockopt", syscall_getsockopt.clone());
            // Do not provide __stack_pointer here; keep the one from generate_emscripten_env
            // invoke_j: return i64, no args
            let invoke_j = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32| -> i64 {
                    if fidx < 0 {
                        return 0;
                    }
                    let idx = fidx as u32;
                    let table = env.data().table.clone();
                    let mut store_mut = env.as_store_mut();
                    if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx)
                    {
                        if let Ok(typed) = func.typed::<(), i64>(&store_mut) {
                            if let Ok(ret) = typed.call(&mut store_mut) {
                                return ret;
                            }
                        }
                    }
                    0
                },
            );
            env_ns.insert("invoke_j", invoke_j);
            let invoke_jii = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32, a1: i32, a2: i32| -> i64 {
                    if fidx < 0 {
                        return 0;
                    }
                    let idx = fidx as u32;
                    let table = env.data().table.clone();
                    let mut store_mut = env.as_store_mut();
                    if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx)
                    {
                        if let Ok(typed) = func.typed::<(i32, i32), i64>(&store_mut) {
                            if let Ok(ret) = typed.call(&mut store_mut, a1, a2) {
                                return ret;
                            }
                        }
                    }
                    0
                },
            );
            env_ns.insert("invoke_jii", invoke_jii.clone());
            let invoke_ji = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32, a1: i32| -> i64 {
                    if fidx < 0 {
                        return 0;
                    }
                    let idx = fidx as u32;
                    let table = env.data().table.clone();
                    let mut store_mut = env.as_store_mut();
                    if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx)
                    {
                        if let Ok(typed) = func.typed::<i32, i64>(&store_mut) {
                            if let Ok(ret) = typed.call(&mut store_mut, a1) {
                                return ret;
                            }
                        }
                    }
                    0
                },
            );
            env_ns.insert("invoke_ji", invoke_ji.clone());
            let invoke_viji = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32, a1: i32, a2: i64, a3: i32| {
                    if fidx < 0 {
                        return;
                    }
                    let idx = fidx as u32;
                    let table = env.data().table.clone();
                    let mut store_mut = env.as_store_mut();
                    if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx)
                    {
                        if let Ok(typed) = func.typed::<(i32, i64, i32), ()>(&store_mut) {
                            let _ = typed.call(&mut store_mut, a1, a2, a3);
                        }
                    }
                },
            );
            env_ns.insert("invoke_viji", invoke_viji.clone());
            let invoke_iiji = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32, a1: i32, a2: i64, a3: i32| -> i32 {
                    if fidx < 0 {
                        return 0;
                    }
                    let idx = fidx as u32;
                    let table = env.data().table.clone();
                    let mut store_mut = env.as_store_mut();
                    if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx)
                    {
                        if let Ok(typed) = func.typed::<(i32, i64, i32), i32>(&store_mut) {
                            if let Ok(ret) = typed.call(&mut store_mut, a1, a2, a3) {
                                return ret;
                            }
                        }
                    }
                    0
                },
            );
            env_ns.insert("invoke_iiji", invoke_iiji.clone());
            let invoke_vj = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32, a1: i64| {
                    if fidx < 0 {
                        return;
                    }
                    let idx = fidx as u32;
                    let table = env.data().table.clone();
                    let mut store_mut = env.as_store_mut();
                    if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx)
                    {
                        if let Ok(typed) = func.typed::<i64, ()>(&store_mut) {
                            let _ = typed.call(&mut store_mut, a1);
                        }
                    }
                },
            );
            env_ns.insert("invoke_vj", invoke_vj.clone());
            let invoke_viiji = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>,
                 fidx: i32,
                 a1: i32,
                 a2: i32,
                 a3: i64,
                 a4: i32| {
                    if fidx < 0 {
                        return;
                    }
                    let idx = fidx as u32;
                    let table = env.data().table.clone();
                    let mut store_mut = env.as_store_mut();
                    if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx)
                    {
                        if let Ok(typed) = func.typed::<(i32, i32, i64, i32), ()>(&store_mut) {
                            let _ = typed.call(&mut store_mut, a1, a2, a3, a4);
                        }
                    }
                },
            );
            env_ns.insert("invoke_viiji", invoke_viiji.clone());
            let invoke_vij = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32, a1: i32, a2: i64| {
                    if fidx < 0 {
                        return;
                    }
                    let idx = fidx as u32;
                    let table = env.data().table.clone();
                    let mut store_mut = env.as_store_mut();
                    if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx)
                    {
                        if let Ok(typed) = func.typed::<(i32, i64), ()>(&store_mut) {
                            let _ = typed.call(&mut store_mut, a1, a2);
                        }
                    }
                },
            );
            env_ns.insert("invoke_vij", invoke_vij.clone());
            let invoke_viij = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32, a1: i32, a2: i32, a3: i64| {
                    if fidx < 0 {
                        return;
                    }
                    let idx = fidx as u32;
                    let table = env.data().table.clone();
                    let mut store_mut = env.as_store_mut();
                    if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx)
                    {
                        if let Ok(typed) = func.typed::<(i32, i32, i64), ()>(&store_mut) {
                            let _ = typed.call(&mut store_mut, a1, a2, a3);
                        }
                    }
                },
            );
            env_ns.insert("invoke_viij", invoke_viij.clone());
            // Provide GOT.mem globals required by the module
            let mut got_mem = Exports::new();
            let heap_base = Global::new_mut(&mut store, Value::I32(0));
            got_mem.insert("__heap_base", heap_base);
            import_object.register_namespace("GOT.mem", got_mem);
            // Register minimal WASI imports now that we have env access
            {
                let mut wasi = Exports::new();
                let write_u32 =
                    |mem: &Memory, store: &mut wasmer::StoreMut<'_>, ptr: u32, val: u32| {
                        if ptr != 0 {
                            let view = mem.view(store);
                            let _ = view.write(ptr as u64, &val.to_le_bytes());
                        }
                    };
                wasi.insert(
                    "environ_sizes_get",
                    Function::new_typed_with_env(
                        &mut store,
                        &invoke_env,
                        move |mut env: FunctionEnvMut<InvokeEnv>, pcount: i32, psize: i32| -> i32 {
                            let mem = env.data().memory.clone();
                            let mut store_mut = env.as_store_mut();
                            write_u32(&mem, &mut store_mut, pcount as u32, 0);
                            write_u32(&mem, &mut store_mut, psize as u32, 0);
                            0
                        },
                    ),
                );
                wasi.insert(
                    "environ_get",
                    Function::new_typed(&mut store, |_a: i32, _b: i32| -> i32 { 0 }),
                );
                wasi.insert(
                    "proc_exit",
                    Function::new_typed(&mut store, |_code: i32| {}),
                );
                wasi.insert(
                    "fd_close",
                    Function::new_typed(&mut store, |_a: i32| -> i32 { 0 }),
                );
                wasi.insert(
                    "fd_sync",
                    Function::new_typed(&mut store, |_a: i32| -> i32 { 0 }),
                );
                wasi.insert(
                    "fd_fdstat_get",
                    Function::new_typed(&mut store, |_a: i32, _b: i32| -> i32 { 0 }),
                );
                wasi.insert(
                    "fd_seek",
                    Function::new_typed_with_env(
                        &mut store,
                        &invoke_env,
                        move |mut env: FunctionEnvMut<InvokeEnv>,
                              _fd: i32,
                              _off: i64,
                              _wh: i32,
                              pout: i32|
                              -> i32 {
                            let mem = env.data().memory.clone();
                            let mut store_mut = env.as_store_mut();
                            write_u32(&mem, &mut store_mut, pout as u32, 0);
                            0
                        },
                    ),
                );
                wasi.insert(
                    "fd_read",
                    Function::new_typed_with_env(
                        &mut store,
                        &invoke_env,
                        move |mut env: FunctionEnvMut<InvokeEnv>,
                              _fd: i32,
                              _iov: i32,
                              _ioc: i32,
                              nread: i32|
                              -> i32 {
                            let mem = env.data().memory.clone();
                            let mut store_mut = env.as_store_mut();
                            write_u32(&mem, &mut store_mut, nread as u32, 0);
                            0
                        },
                    ),
                );
                wasi.insert(
                    "fd_write",
                    Function::new_typed_with_env(
                        &mut store,
                        &invoke_env,
                        move |mut env: FunctionEnvMut<InvokeEnv>,
                              _fd: i32,
                              _iov: i32,
                              _ioc: i32,
                              nw: i32|
                              -> i32 {
                            let mem = env.data().memory.clone();
                            let mut store_mut = env.as_store_mut();
                            write_u32(&mem, &mut store_mut, nw as u32, u32::MAX);
                            0
                        },
                    ),
                );
                wasi.insert(
                    "fd_pread",
                    Function::new_typed_with_env(
                        &mut store,
                        &invoke_env,
                        move |mut env: FunctionEnvMut<InvokeEnv>,
                              _fd: i32,
                              _iov: i32,
                              _ioc: i32,
                              _off: i64,
                              nread: i32|
                              -> i32 {
                            let mem = env.data().memory.clone();
                            let mut store_mut = env.as_store_mut();
                            write_u32(&mem, &mut store_mut, nread as u32, 0);
                            0
                        },
                    ),
                );
                wasi.insert(
                    "fd_pwrite",
                    Function::new_typed_with_env(
                        &mut store,
                        &invoke_env,
                        move |mut env: FunctionEnvMut<InvokeEnv>,
                              _fd: i32,
                              _iov: i32,
                              _ioc: i32,
                              _off: i64,
                              nw: i32|
                              -> i32 {
                            let mem = env.data().memory.clone();
                            let mut store_mut = env.as_store_mut();
                            write_u32(&mem, &mut store_mut, nw as u32, u32::MAX);
                            0
                        },
                    ),
                );
                wasi.insert(
                    "clock_time_get",
                    Function::new_typed_with_env(
                        &mut store,
                        &invoke_env,
                        move |mut env: FunctionEnvMut<InvokeEnv>,
                              _clock: i32,
                              _prec: i64,
                              tp: i32|
                              -> i32 {
                            if tp != 0 {
                                let mem = env.data().memory.clone();
                                let store_mut = env.as_store_mut();
                                let view = mem.view(&store_mut);
                                let _ = view.write(tp as u64, &0u64.to_le_bytes());
                            }
                            0
                        },
                    ),
                );
                wasi.insert(
                    "random_get",
                    Function::new_typed_with_env(
                        &mut store,
                        &invoke_env,
                        move |mut env: FunctionEnvMut<InvokeEnv>, buf: i32, len: i32| -> i32 {
                            if len > 0 && buf != 0 {
                                let mem = env.data().memory.clone();
                                let store_mut = env.as_store_mut();
                                let view = mem.view(&store_mut);
                                let tmp = vec![0u8; len as usize];
                                let _ = view.write(buf as u64, &tmp);
                            }
                            0
                        },
                    ),
                );
                import_object.register_namespace("wasi_snapshot_preview1", wasi);
                eprintln!("[pglite] registered wasi_snapshot_preview1 imports");
            }
            let syscall_sendto = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>,
                 fd: i32,
                 buf: i32,
                 len: i32,
                 _flags: i32,
                 _addr: i32,
                 _alen: i32|
                 -> i32 {
                    if len <= 0 {
                        return 0;
                    }
                    let l = len as usize;
                    let mut tmp = vec![0u8; l];
                    {
                        let memory = env.data().memory.clone();
                        let store_mut = env.as_store_mut();
                        let view = memory.view(&store_mut);
                        let mem_len = view.data_size() as u64;
                        let off = buf as u64;
                        let mut nn = l as u64;
                        if off >= mem_len {
                            nn = 0;
                        } else if off + nn > mem_len {
                            nn = mem_len - off;
                        }
                        if nn > 0 {
                            let _ = view.read(off, &mut tmp[..(nn as usize)]);
                        }
                    }
                    if let Some(q) = env.data_mut().sockets.get_mut(&fd) {
                        for b in &tmp {
                            q.push_back(*b);
                        }
                        return len;
                    }
                    len
                },
            );
            env_ns.insert("__syscall_sendto", syscall_sendto.clone());
            let syscall_recvfrom = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>,
                 fd: i32,
                 buf: i32,
                 len: i32,
                 _flags: i32,
                 _addr: i32,
                 _alen: i32|
                 -> i32 {
                    if len <= 0 {
                        return 0;
                    }
                    let l = len as usize;
                    let mut out = vec![0u8; l];
                    let mut n = 0usize;
                    if let Some(q) = env.data_mut().sockets.get_mut(&fd) {
                        while n < l {
                            if let Some(b) = q.pop_front() {
                                out[n] = b;
                                n += 1;
                            } else {
                                break;
                            }
                        }
                    }
                    if n > 0 {
                        let memory = env.data().memory.clone();
                        let store_mut = env.as_store_mut();
                        let view = memory.view(&store_mut);
                        let mem_len = view.data_size() as u64;
                        let off = buf as u64;
                        let mut nn = n as u64;
                        if off >= mem_len {
                            nn = 0;
                        } else if off + nn > mem_len {
                            nn = mem_len - off;
                        }
                        if nn > 0 {
                            let _ = view.write(off, &out[..(nn as usize)]);
                            return nn as i32;
                        }
                    }
                    0
                },
            );
            env_ns.insert("__syscall_recvfrom", syscall_recvfrom.clone());
            let syscall_poll =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32| -> i32 { 0 });
            env_ns.insert("__syscall_poll", syscall_poll.clone());
            let syscall_fadvise64 =
                Function::new_typed(&mut store, |_a: i32, _b: i64, _c: i64, _d: i32| -> i32 {
                    0
                });
            env_ns.insert("__syscall_fadvise64", syscall_fadvise64);
            let syscall_fallocate =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i64, _d: i64| -> i32 {
                    0
                });
            env_ns.insert("__syscall_fallocate", syscall_fallocate);
            let em_get_progname = Function::new_typed(&mut store, |_a: i32, _b: i32| {});
            env_ns.insert("_emscripten_get_progname", em_get_progname);
            let em_keepalive_clear = Function::new_typed(&mut store, || {});
            env_ns.insert("_emscripten_runtime_keepalive_clear", em_keepalive_clear);
            let em_lookup_name = Function::new_typed(&mut store, |_a: i32| -> i32 { 0 });
            env_ns.insert("_emscripten_lookup_name", em_lookup_name.clone());
            let emscripten_date_now = Function::new_typed(&mut store, || -> f64 { 0.0 });
            env_ns.insert("emscripten_date_now", emscripten_date_now.clone());
            let emscripten_get_now = Function::new_typed(&mut store, || -> f64 { 0.0 });
            env_ns.insert("emscripten_get_now", emscripten_get_now.clone());
            let call_sighandler = Function::new_typed(&mut store, |_a: i32, _b: i32| {});
            env_ns.insert("__call_sighandler", call_sighandler);
            let syscall_readlinkat =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32, _d: i32| -> i32 {
                    0
                });
            env_ns.insert("__syscall_readlinkat", syscall_readlinkat);
            let syscall_fdatasync = Function::new_typed(&mut store, |_a: i32| -> i32 { 0 });
            env_ns.insert("__syscall_fdatasync", syscall_fdatasync.clone());
            let syscall_fstat64 = Function::new_typed(&mut store, |_a: i32, _b: i32| -> i32 { 0 });
            env_ns.insert("__syscall_fstat64", syscall_fstat64.clone());
            let syscall_stat64 = Function::new_typed(&mut store, |_a: i32, _b: i32| -> i32 { 0 });
            env_ns.insert("__syscall_stat64", syscall_stat64.clone());
            let syscall_newfstatat =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32, _d: i32| -> i32 {
                    0
                });
            env_ns.insert("__syscall_newfstatat", syscall_newfstatat.clone());
            let syscall_lstat64 = Function::new_typed(&mut store, |_a: i32, _b: i32| -> i32 { 0 });
            env_ns.insert("__syscall_lstat64", syscall_lstat64.clone());
            let syscall_ftruncate64 =
                Function::new_typed(&mut store, |_a: i32, _b: i64| -> i32 { 0 });
            env_ns.insert("__syscall_ftruncate64", syscall_ftruncate64.clone());
            let syscall_truncate64 =
                Function::new_typed(&mut store, |_a: i32, _b: i64| -> i32 { 0 });
            env_ns.insert("__syscall_truncate64", syscall_truncate64.clone());
            let syscall_getcwd = Function::new_typed(&mut store, |_a: i32, _b: i32| -> i32 { 0 });
            env_ns.insert("__syscall_getcwd", syscall_getcwd.clone());
            let syscall_getdents64 =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32| -> i32 { 0 });
            env_ns.insert("__syscall_getdents64", syscall_getdents64.clone());
            let syscall_mkdirat =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32| -> i32 { 0 });
            env_ns.insert("__syscall_mkdirat", syscall_mkdirat.clone());
            let syscall_unlinkat =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32| -> i32 { 0 });
            env_ns.insert("__syscall_unlinkat", syscall_unlinkat);
            let syscall_rmdir = Function::new_typed(&mut store, |_a: i32| -> i32 { 0 });
            env_ns.insert("__syscall_rmdir", syscall_rmdir);
            let syscall_renameat =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32, _d: i32| -> i32 {
                    0
                });
            env_ns.insert("__syscall_renameat", syscall_renameat);
            let syscall_newselect = Function::new_typed(
                &mut store,
                |_a: i32, _b: i32, _c: i32, _d: i32, _e: i32| -> i32 { 0 },
            );
            env_ns.insert("__syscall__newselect", syscall_newselect);
            let setitimer_js = Function::new_typed(&mut store, |_a: i32, _b: f64| -> i32 { 0 });
            env_ns.insert("_setitimer_js", setitimer_js);
            let syscall_symlinkat =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32| -> i32 { 0 });
            env_ns.insert("__syscall_symlinkat", syscall_symlinkat);
            let get_heap_max = Function::new_typed(&mut store, || -> i32 { i32::MAX });
            env_ns.insert("emscripten_get_heap_max", get_heap_max);
            let em_system = Function::new_typed(&mut store, |_a: i32| -> i32 { 0 });
            env_ns.insert("_emscripten_system", em_system);
            let syscall_truncate64 =
                Function::new_typed(&mut store, |_a: i32, _b: i64| -> i32 { 0 });
            env_ns.insert("__syscall_truncate64", syscall_truncate64.clone());
            let em_throw_longjmp = Function::new_typed(&mut store, || {});
            env_ns.insert("_emscripten_throw_longjmp", em_throw_longjmp);
            let syscall_accept4 = Function::new_typed(
                &mut store,
                |_a: i32, _b: i32, _c: i32, _d: i32, _e: i32, _f: i32| -> i32 { 0 },
            );
            env_ns.insert("__syscall_accept4", syscall_accept4.clone());
            let syscall_bind = Function::new_typed(
                &mut store,
                |_a: i32, _b: i32, _c: i32, _d: i32, _e: i32, _f: i32| -> i32 { 0 },
            );
            env_ns.insert("__syscall_bind", syscall_bind);
        }
        // invoke_ij: return i32, args: i64
        let invoke_ij = Function::new_typed_with_env(
            &mut store,
            &invoke_env,
            |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32, a1: i64| -> i32 {
                if fidx < 0 {
                    return 0;
                }
                let idx = fidx as u32;
                let table = env.data().table.clone();
                let mut store_mut = env.as_store_mut();
                if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx) {
                    if let Ok(typed) = func.typed::<i64, i32>(&store_mut) {
                        if let Ok(ret) = typed.call(&mut store_mut, a1) {
                            return ret;
                        }
                    }
                }
                0
            },
        );
        env_ns.insert("invoke_ij", invoke_ij.clone());
        // Optionally reduce shims to a minimal set to isolate instantiation issues.
        #[cfg(any())]
        if std::env::var("PGLITE_MIN_ENV").ok().as_deref() == Some("1") {
            let mut minimal = Exports::new();
            minimal.insert(
                "__stack_pointer",
                Global::new_mut(&mut store, Value::I32(0)),
            );
            minimal.insert("__indirect_function_table", globals.table.clone());
            // Common Emscripten helpers
            let asm_const_int =
                Function::new_typed(&mut store, |_a: i32, _b: i32, _c: i32| -> i32 { 0 });
            minimal.insert("emscripten_asm_const_int", asm_const_int);
            let force_exit = Function::new_typed(&mut store, |_code: i32| {});
            minimal.insert("emscripten_force_exit", force_exit);
            // Provide only the invoke_* shims that use i64 ("j") to match
            // Emscripten's Node target expectations.
            let invoke_vji = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>, fidx: i32, a1: i32, a2: i64, a3: i32| {
                    if fidx < 0 {
                        return;
                    }
                    let idx = fidx as u32;
                    let table = env.data().table.clone();
                    let mut store_mut = env.as_store_mut();
                    if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx)
                    {
                        if let Ok(typed) = func.typed::<(i32, i64, i32), ()>(&store_mut) {
                            let _ = typed.call(&mut store_mut, a1, a2, a3);
                        }
                    }
                },
            );
            minimal.insert("invoke_vji", invoke_vji);
            let invoke_jiiiiiiiii = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>,
                 fidx: i32,
                 a1: i32,
                 a2: i32,
                 a3: i32,
                 a4: i32,
                 a5: i32,
                 a6: i32,
                 a7: i32,
                 a8: i32,
                 a9: i32|
                 -> i64 {
                    if fidx < 0 {
                        return 0;
                    }
                    let idx = fidx as u32;
                    let table = env.data().table.clone();
                    let mut store_mut = env.as_store_mut();
                    if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx)
                    {
                        if let Ok(typed) = func
                            .typed::<(i32, i32, i32, i32, i32, i32, i32, i32, i32), i64>(&store_mut)
                        {
                            if let Ok(ret) =
                                typed.call(&mut store_mut, a1, a2, a3, a4, a5, a6, a7, a8, a9)
                            {
                                return ret;
                            }
                        }
                    }
                    0
                },
            );
            minimal.insert("invoke_jiiiiiiiii", invoke_jiiiiiiiii);
            let invoke_jiiiiii = Function::new_typed_with_env(
                &mut store,
                &invoke_env,
                |mut env: FunctionEnvMut<InvokeEnv>,
                 fidx: i32,
                 a1: i32,
                 a2: i32,
                 a3: i32,
                 a4: i32,
                 a5: i32,
                 a6: i32|
                 -> i64 {
                    if fidx < 0 {
                        return 0;
                    }
                    let idx = fidx as u32;
                    let table = env.data().table.clone();
                    let mut store_mut = env.as_store_mut();
                    if let Some(wasmer::Value::FuncRef(Some(func))) = table.get(&mut store_mut, idx)
                    {
                        if let Ok(typed) =
                            func.typed::<(i32, i32, i32, i32, i32, i32), i64>(&store_mut)
                        {
                            if let Ok(ret) = typed.call(&mut store_mut, a1, a2, a3, a4, a5, a6) {
                                return ret;
                            }
                        }
                    }
                    0
                },
            );
            minimal.insert("invoke_jiiiiii", invoke_jiiiiii);
            minimal.insert("invoke_iiiiiiiiiiiiii", invoke_iiiiiiiiiiiiii);
            minimal.insert("invoke_iiiijii", invoke_iiiijii);
            minimal.insert("invoke_vijiji", invoke_vijiji);
            minimal.insert("invoke_viji", invoke_viji);
            minimal.insert("invoke_iiji", invoke_iiji);
            minimal.insert("invoke_iiiij", invoke_iiiij);
            minimal.insert("invoke_viiiji", invoke_viiiji);
            minimal.insert("invoke_vid", invoke_vid);
            minimal.insert("invoke_ijiiiiii", invoke_ijiiiiii);
            minimal.insert("invoke_viijii", invoke_viijii);
            minimal.insert("invoke_iiiiiji", invoke_iiiiiji);
            minimal.insert("invoke_viijiiii", invoke_viijiiii);
            minimal.insert("invoke_viiiiiiiiiiii", invoke_viiiiiiiiiiii);
            minimal.insert("invoke_di", invoke_di);
            minimal.insert("invoke_id", invoke_id);
            minimal.insert("invoke_ijiiiii", invoke_ijiiiii);
            // Minimal syscall and env shims needed by the module
            minimal.insert("__syscall_fcntl64", syscall_fcntl64.clone());
            minimal.insert("__syscall_ioctl", syscall_ioctl.clone());
            minimal.insert("__syscall_openat", syscall_openat.clone());
            minimal.insert("__syscall_faccessat", syscall_faccessat.clone());
            minimal.insert("__syscall_chdir", syscall_chdir.clone());
            minimal.insert("__syscall_chmod", syscall_chmod.clone());
            minimal.insert("__syscall_dup", syscall_dup.clone());
            minimal.insert("__syscall_dup3", syscall_dup3.clone());
            minimal.insert("_dlopen_js", dlopen_js.clone());
            minimal.insert("_dlsym_js", dlsym_js.clone());
            minimal.insert("_emscripten_memcpy_js", memcpy_js.clone());
            minimal.insert("_munmap_js", munmap_js.clone());
            minimal.insert("_mmap_js", mmap_js.clone());
            minimal.insert("__syscall_pipe", syscall_pipe.clone());
            minimal.insert("__syscall_read", syscall_read.clone());
            minimal.insert("__syscall_write", syscall_write.clone());
            minimal.insert("__syscall_close", syscall_close.clone());
            minimal.insert("__syscall_socket", syscall_socket.clone());
            minimal.insert("__syscall_connect", syscall_connect.clone());
            minimal.insert("__syscall_listen", syscall_listen.clone());
            minimal.insert("__syscall_accept4", syscall_accept4.clone());
            minimal.insert("__syscall_getsockname", syscall_getsockname.clone());
            minimal.insert("__syscall_getsockopt", syscall_getsockopt.clone());
            minimal.insert("__syscall_sendto", syscall_sendto.clone());
            minimal.insert("__syscall_recvfrom", syscall_recvfrom.clone());
            minimal.insert("__syscall_poll", syscall_poll.clone());
            minimal.insert("_tzset_js", tzset_js.clone());
            minimal.insert("_abort_js", abort_js.clone());
            minimal.insert("emscripten_date_now", emscripten_date_now.clone());
            minimal.insert("emscripten_get_now", emscripten_get_now.clone());
            minimal.insert("__syscall_fdatasync", syscall_fdatasync.clone());
            minimal.insert("__syscall_fstat64", syscall_fstat64.clone());
            minimal.insert("__syscall_stat64", syscall_stat64.clone());
            minimal.insert("__syscall_newfstatat", syscall_newfstatat.clone());
            minimal.insert("__syscall_lstat64", syscall_lstat64.clone());
            minimal.insert("__syscall_ftruncate64", syscall_ftruncate64.clone());
            minimal.insert("__syscall_truncate64", syscall_truncate64.clone());
            minimal.insert("__syscall_getcwd", syscall_getcwd.clone());
            minimal.insert("__syscall_getdents64", syscall_getdents64.clone());
            minimal.insert("__syscall_mkdirat", syscall_mkdirat.clone());
            minimal.insert("_emscripten_lookup_name", em_lookup_name.clone());
            minimal.insert("_localtime_js", localtime_js.clone());
            minimal.insert("_gmtime_js", gmtime_js.clone());
            minimal.insert("invoke_vj", invoke_vj);
            minimal.insert("invoke_viiji", invoke_viiji);
            minimal.insert("invoke_vij", invoke_vij);
            minimal.insert("invoke_ij", invoke_ij);
            minimal.insert("invoke_iiij", invoke_iiij);
            minimal.insert("invoke_jiiii", invoke_jiiii);
            minimal.insert("invoke_jii", invoke_jii);
            minimal.insert("invoke_ji", invoke_ji);
            minimal.insert("invoke_iiiiiiiiiiiiiiiiii", invoke_iiiiiiiiiiiiiiiiii);
            env_ns = minimal;
        }
        // Merge env: override invoke_*; insert shims only if missing; never override memory or table
        // Extend the import object with our shims under the "env" namespace.
        let mut shim_imports = Imports::new();
        shim_imports.register_namespace("env", env_ns);
        import_object.extend(&shim_imports);
        eprintln!("[pglite] shims merged; instantiating");
        let instance = Instance::new(&mut store, &module, &import_object)?;
        eprintln!("[pglite] instance created");
        // Debug export listing removed to reduce log noise
        // Skip running Emscripten constructors; module APIs handle init paths.

        if std::env::var("PGLITE_SKIP_INITDB").ok().as_deref() != Some("1") {
            // Call pgl_initdb to ensure the database files are set up.
            let init = instance
                .exports
                .get_typed_function::<(), i32>(&mut store, "pgl_initdb")?;
            eprintln!("[pglite] calling pgl_initdb");
            let rc = init.call(&mut store)?;
            eprintln!("[pglite] pgl_initdb returned {rc}");
            if rc != 0 {
                return Err(anyhow!("pglite initdb failed with code {rc}"));
            }
        } else {
            eprintln!("[pglite] skipping _pgl_initdb by env");
        }

        let interactive_write = instance
            .exports
            .get_typed_function::<i32, ()>(&mut store, "interactive_write")?;
        let interactive_read = instance
            .exports
            .get_typed_function::<(), i32>(&mut store, "interactive_read")?;
        let get_channel = instance
            .exports
            .get_typed_function::<(), i32>(&mut store, "get_channel")?;
        let use_wire = instance
            .exports
            .get_typed_function::<i32, ()>(&mut store, "use_wire")?;
        let get_buffer_size = instance
            .exports
            .get_function("get_buffer_size")
            .ok()
            .cloned();
        let get_buffer_addr = instance
            .exports
            .get_function("get_buffer_addr")
            .ok()
            .cloned();
        let backend = instance
            .exports
            .get_typed_function::<(), ()>(&mut store, "pgl_backend")?;
        let shutdown_fn = instance
            .exports
            .get_typed_function::<(), ()>(&mut store, "pgl_shutdown")?;

        Ok(Self {
            store,
            _instance: instance,
            memory: globals.memory.clone(),
            interactive_write,
            interactive_read,
            get_channel,
            get_buffer_size,
            get_buffer_addr,
            use_wire,
            backend,
            shutdown_fn,
            _table: globals.table.clone(),
        })
    }

    /// Execute a single protocol message and return the backend response bytes.
    fn exec_protocol(&mut self, message: &[u8]) -> Result<Vec<u8>> {
        self.use_wire.call(&mut self.store, 1)?;
        // Prepare write
        self.interactive_write
            .call(&mut self.store, message.len() as i32)?;
        // Determine buffer base and capacity
        let (base, cap) = if let (Some(ref addr_fn), Some(ref size_fn)) =
            (self.get_buffer_addr.as_ref(), self.get_buffer_size.as_ref())
        {
            let addr_val = addr_fn
                .call(&mut self.store, &[Value::I32(0)])
                .map_err(|e| anyhow!(e.to_string()))?
                .get(0)
                .cloned()
                .unwrap_or(Value::I32(0));
            let size_val = size_fn
                .call(&mut self.store, &[Value::I32(0)])
                .map_err(|e| anyhow!(e.to_string()))?
                .get(0)
                .cloned()
                .unwrap_or(Value::I32(0));
            let addr_u64 = match addr_val {
                Value::I32(x) => x as u64,
                Value::I64(x) => x as u64,
                _ => 0,
            };
            let size_usize = match size_val {
                Value::I32(x) => x as usize,
                Value::I64(x) => x as usize,
                _ => 0,
            };
            (addr_u64, size_usize)
        } else {
            // Fallback to legacy layout assumptions
            let total = self.memory.view(&self.store).data_size();
            (1u64, total.saturating_sub(2) as usize)
        };
        if message.len() > cap {
            return Err(anyhow!(
                "message too large for buffer ({} > {})",
                message.len(),
                cap
            ));
        }
        {
            let view = self.memory.view(&self.store);
            let mem_len = view.data_size() as u64;
            if base + (message.len() as u64) > mem_len {
                return Err(anyhow!("buffer write out of bounds"));
            }
            view.write(base, message)
                .map_err(|e| anyhow!(e.to_string()))?;
        }
        self.backend.call(&mut self.store)?;
        let chan = self.get_channel.call(&mut self.store)?;
        if chan <= 0 {
            return Err(anyhow!("unsupported channel"));
        }
        let out_len = self.interactive_read.call(&mut self.store)? as usize;
        if out_len > cap {
            return Err(anyhow!(
                "response too large for buffer ({} > {})",
                out_len,
                cap
            ));
        }
        let mut out = vec![0u8; out_len];
        {
            let view = self.memory.view(&self.store);
            let mem_len = view.data_size() as u64;
            // If we don't know base, assume response directly follows request + 2 bytes header
            let read_base = if self.get_buffer_addr.is_some() {
                base
            } else {
                (message.len() + 2) as u64
            };
            if read_base + (out_len as u64) > mem_len {
                return Err(anyhow!("response length out of bounds"));
            }
            view.read(read_base, &mut out)
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
