# fmt

Format HCL files in place for consistent style.

## Usage

```bash
dbschema fmt              # format current directory recursively
dbschema fmt path/to.hcl  # format a single file
dbschema fmt dir/         # format all .hcl files under dir/
```

## Options

- `PATH` (positional, default `.`): File or directory to format. Can be specified multiple times.

## Notes

- Only `.hcl` files are formatted; other files are ignored.
- The formatter parses HCL and writes a normalized representation.
