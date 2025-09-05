use std::path::PathBuf;

fn main() {
    let wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("node_modules/@electric-sql/pglite/dist/pglite.wasm");
    let bytes = std::fs::read(&wasm_path).expect("read wasm");
    println!("Imports:");
    let mut parser = wasmparser::Parser::new(0);
    for payload in parser.parse_all(&bytes) {
        match payload.expect("payload") {
            wasmparser::Payload::ImportSection(s) => {
                for import in s {
                    let import = import.expect("import");
                    println!("  {}.{}: {:?}", import.module, import.name, import.ty);
                }
            }
            _ => {}
        }
    }
}

