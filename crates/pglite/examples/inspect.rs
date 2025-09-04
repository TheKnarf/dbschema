use std::path::PathBuf;
use wasmer::{Module, Store};

fn main() {
    let wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("node_modules/@electric-sql/pglite/dist/pglite.wasm");
    let bytes = std::fs::read(&wasm_path).expect("read wasm");
    let mut store = Store::default();
    let module = Module::new(&store, bytes).expect("module");
    println!("Imports:");
    for imp in module.imports() {
        println!("  {}.{} : {:?}", imp.module(), imp.name(), imp.ty());
    }
    println!("Exports:");
    for exp in module.exports() {
        println!("  {} : {:?}", exp.name(), exp.ty());
    }
}
