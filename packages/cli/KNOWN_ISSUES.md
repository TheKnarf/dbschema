# Known Issues

## WASM Path Handling Bug

**Status:** Blocking - CLI is not functional

**Issue:** The WASM module is calling the JavaScript file loader callback with `undefined` instead of the file path string.

**Symptoms:**
```
[DEBUG] loadFile called with path: undefined
Error: File path is empty or undefined (received: undefined)
```

**What we know:**
1. JavaScript is correctly calling `validate_hcl()` with:
   - Absolute path: `/Users/.../examples/table.hcl` ✅
   - Loader function: `function` ✅
   - Options object: Valid WASM pointer ✅

2. The loader callback IS being called, but with `undefined` as the path argument

3. Rust debug output from `validate_hcl` entry point is NOT appearing, suggesting the error happens very early

4. The wasm-bindgen glue code shows: `arg1.call(getStringFromWasm0(arg2, arg3))` where `arg2, arg3` are pointer/length

**Hypothesis:**
The issue is likely in how `path.display().to_string()` in Rust is being marshalled through wasm-bindgen's `call(this: &LoaderCallback, path: &str)`. The string may not be getting copied to WASM linear memory correctly, or the pointer/length are invalid.

**Next Steps to Debug:**
1. Add more tracing in wasm-bindgen glue code to see actual `arg2, arg3` values
2. Try using a simpler string passing mechanism (maybe just pass the string directly without Path)
3. Check if the issue is specific to `Path::display().to_string()` vs just a regular `String`
4. Verify WASM memory initialization is correct

**Workaround:**
None currently - the Node CLI is not functional. Users must use the native Rust CLI (`cargo install dbschema`).

## Missing Features in WASM

These are intentional limitations:

- ❌ `test` command - Requires PostgreSQL connection
- ❌ `lint` `sql-syntax` check - Requires pg_query native library

These features will never be available in the WASM version.