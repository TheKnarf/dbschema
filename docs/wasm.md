# Building dbschema for WebAssembly

This guide explains how to build the dbschema library as a WebAssembly module for use in JavaScript/TypeScript projects.

## Prerequisites

### 1. Install Rust

If you haven't already installed Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 2. Add WASM Target

Add the WebAssembly compilation target to your Rust installation:

```bash
rustup target add wasm32-unknown-unknown
```

### 3. Install wasm-pack

[wasm-pack](https://rustwasm.github.io/wasm-pack/) is the recommended tool for building Rust-generated WebAssembly:

```bash
cargo install wasm-pack
```

## Building

### For Node.js

Build the library for Node.js environments:

```bash
wasm-pack build --target nodejs --features wasm --no-default-features
```

This creates a `pkg/` directory with:
- `dbschema_bg.wasm` - The WebAssembly binary
- `dbschema.js` - JavaScript wrapper
- `dbschema.d.ts` - TypeScript type definitions
- `package.json` - NPM package metadata

### For Web Browsers

Build for browser environments:

```bash
wasm-pack build --target web --features wasm --no-default-features
```

### For Bundlers (Webpack, Rollup, etc.)

Build for bundler environments:

```bash
wasm-pack build --target bundler --features wasm --no-default-features
```

## Usage Examples

### Node.js

```javascript
const fs = require('fs');
const { validate_hcl, generate, format_hcl, ValidateOptions, GenerateOptions } = require('./pkg');

// File loader callback
function loadFile(path) {
  return fs.readFileSync(path, 'utf-8');
}

// Validate HCL
const validateOpts = new ValidateOptions();
validateOpts.strict = true;

const result = validate_hcl('main.hcl', loadFile, validateOpts);
if (result.success) {
  console.log(result.summary);
} else {
  console.error(result.error);
}

// Generate SQL
const genOpts = new GenerateOptions('postgres');
genOpts.strict = false;
genOpts.include_resources = ['tables', 'functions'];

const sql = generate('main.hcl', loadFile, genOpts);
console.log(sql);

// Format HCL
const formatted = format_hcl('table "users" { column "id" { type = "serial" } }');
console.log(formatted);
```

### TypeScript

```typescript
import * as fs from 'fs';
import { validate_hcl, generate, format_hcl, ValidateOptions, GenerateOptions } from './pkg';

// File loader callback
function loadFile(path: string): string {
  return fs.readFileSync(path, 'utf-8');
}

// Validate HCL
const validateOpts = new ValidateOptions();
validateOpts.strict = true;

const result = validate_hcl('main.hcl', loadFile, validateOpts);
if (result.success) {
  console.log(result.summary);
} else {
  console.error(result.error);
}

// Generate SQL
const genOpts = new GenerateOptions('postgres');
genOpts.strict = false;
genOpts.include_resources = ['tables', 'functions'];

try {
  const sql = generate('main.hcl', loadFile, genOpts);
  console.log(sql);
} catch (error) {
  console.error('Generation failed:', error);
}
```

### Browser

```html
<!DOCTYPE html>
<html>
<head>
  <script type="module">
    import init, { validate_hcl, generate, format_hcl, ValidateOptions, GenerateOptions } from './pkg/dbschema.js';

    async function run() {
      // Initialize the WASM module
      await init();

      // File loader - in browser you might fetch from URLs or use a virtual filesystem
      function loadFile(path) {
        // Example: load from a virtual filesystem object
        const files = {
          'main.hcl': `
            table "users" {
              column "id" { type = "serial" }
              column "name" { type = "text" }
            }
          `
        };
        return files[path] || '';
      }

      // Validate
      const validateOpts = new ValidateOptions();
      const result = validate_hcl('main.hcl', loadFile, validateOpts);

      if (result.success) {
        console.log('Validation passed:', result.summary);

        // Generate SQL
        const genOpts = new GenerateOptions('postgres');
        const sql = generate('main.hcl', loadFile, genOpts);
        console.log('Generated SQL:', sql);
      } else {
        console.error('Validation failed:', result.error);
      }
    }

    run().catch(console.error);
  </script>
</head>
<body>
  <h1>dbschema WASM Demo</h1>
  <p>Check the console for output</p>
</body>
</html>
```

## API Reference

### Functions

#### `validate_hcl(root_path, loader, options?)`

Validates HCL configuration.

- **root_path**: `string` - Path to the root HCL file
- **loader**: `(path: string) => string` - Callback function to load files
- **options**: `ValidateOptions` (optional) - Validation options
- **Returns**: `ValidateResult` with `success`, `error`, and `summary` properties

#### `generate(root_path, loader, options)`

Generates SQL or other backend output.

- **root_path**: `string` - Path to the root HCL file
- **loader**: `(path: string) => string` - Callback function to load files
- **options**: `GenerateOptions` - Generation options (backend type, etc.)
- **Returns**: `string` - Generated output
- **Throws**: Error if generation fails

#### `format_hcl(content)`

Formats HCL content.

- **content**: `string` - HCL content to format
- **Returns**: `string` - Formatted HCL
- **Throws**: Error if parsing fails

#### `version()`

Returns the library version.

- **Returns**: `string` - Version string

### Classes

#### `ValidateOptions`

Options for validation.

```typescript
class ValidateOptions {
  constructor();
  strict: boolean;
  include_resources: string[];
  exclude_resources: string[];
}
```

#### `GenerateOptions`

Options for generation.

```typescript
class GenerateOptions {
  constructor(backend: string);
  strict: boolean;
  include_resources: string[];
  exclude_resources: string[];
}
```

**Supported backends**:
- `postgres` - PostgreSQL SQL
- `json` - JSON representation
- `prisma` - Prisma schema

**Resource types** (for include/exclude):
- `schemas`, `enums`, `domains`, `types`, `tables`, `views`, `materialized`
- `functions`, `procedures`, `aggregates`, `operators`, `triggers`, `rules`
- `event_triggers`, `extensions`, `collations`, `sequences`, `policies`
- `roles`, `tablespaces`, `grants`, `tests`, `indexes`
- `publications`, `subscriptions`
- `foreign_data_wrappers`, `foreign_servers`, `foreign_tables`
- `text_search_dictionaries`, `text_search_configurations`

## Features and Limitations

### Available in WASM

✅ HCL parsing and validation
✅ SQL generation (PostgreSQL)
✅ Prisma schema generation
✅ JSON output
✅ HCL formatting
✅ All linting rules except `sql-syntax`

### Not Available in WASM

❌ `test` command (requires PostgreSQL connection)
❌ `sql-syntax` lint rule (requires pg_query native library)

These features require the native Rust CLI built with the `postgres-backend` feature.

## Publishing to NPM

### 1. Build the package

```bash
wasm-pack build --target nodejs --features wasm --no-default-features --scope your-org
```

### 2. Test locally

```bash
cd pkg
npm link
cd /path/to/your/project
npm link @your-org/dbschema
```

### 3. Publish

```bash
cd pkg
npm publish --access public
```

## Development

### Build native CLI (with all features)

```bash
cargo build --release
```

### Build WASM-only (no postgres features)

```bash
cargo build --features wasm --no-default-features
```

### Run tests

```bash
# Run all tests with default features
cargo test

# Run tests without postgres features
cargo test --no-default-features --features wasm
```

## Troubleshooting

### Error: "Cannot find module 'wasm-pack'"

Install wasm-pack: `cargo install wasm-pack`

### Error: "wasm32-unknown-unknown target not found"

Add the target: `rustup target add wasm32-unknown-unknown`

### Build fails with linker errors

Make sure you're using `--no-default-features --features wasm` to disable postgres dependencies.

### WASM module is too large

The WASM binary includes the HCL parser and code generation logic. Consider:
- Using `wasm-opt` to optimize: `wasm-opt -Oz input.wasm -o output.wasm`
- Enabling compression in your web server (gzip/brotli)
- Using dynamic imports to load WASM on-demand

## Additional Resources

- [wasm-pack documentation](https://rustwasm.github.io/wasm-pack/)
- [Rust and WebAssembly book](https://rustwasm.github.io/docs/book/)
- [wasm-bindgen guide](https://rustwasm.github.io/wasm-bindgen/)