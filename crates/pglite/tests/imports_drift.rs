use std::collections::HashSet;
use std::path::PathBuf;

// Lightweight import-set drift test that avoids linking Wasmer.
// It parses the WASM with `wasmparser` and asserts imports only
// come from the expected modules.
#[test]
fn wasm_import_modules_stable() {
    let pkg_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("node_modules/@electric-sql/pglite/dist");
    let wasm_path = pkg_dir.join("pglite.wasm");

    if !wasm_path.exists() {
        eprintln!(
            "skipping: missing {} (run `just pglite-assets` in workspace root)",
            wasm_path.display()
        );
        return;
    }

    let bytes = std::fs::read(&wasm_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", wasm_path.display()));

    let mut modules: HashSet<String> = HashSet::new();
    let parser = wasmparser::Parser::new(0);
    for payload in parser.parse_all(&bytes) {
        match payload.expect("parse payload") {
            wasmparser::Payload::ImportSection(s) => {
                for import in s {
                    let import = import.expect("import");
                    modules.insert(import.module.to_string());
                }
            }
            _ => {}
        }
    }

    // Expected import module names for the current PGlite build.
    let allowed: HashSet<&str> = ["env", "wasi_snapshot_preview1", "GOT.mem"]
        .into_iter()
        .collect();
    let required: HashSet<&str> = ["env", "wasi_snapshot_preview1"].into_iter().collect();

    // Detect any unexpected modules.
    let unknown: Vec<String> = modules
        .iter()
        .filter(|m| !allowed.contains(m.as_str()))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        panic!(
            "unexpected import modules: {} (full set: {:?})",
            unknown.join(", "),
            modules
        );
    }

    // Ensure core modules are present.
    for req in &required {
        assert!(modules.contains(*req), "missing required module: {req}");
    }
}
