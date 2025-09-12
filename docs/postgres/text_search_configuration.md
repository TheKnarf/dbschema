# Text Search Configuration

Defines a full text search configuration and optional mappings.

```hcl
text_search_configuration "english" {
  parser = "default"

  mapping {
    for  = ["asciiword"]
    with = ["english_stem"]
  }
}
```

## Attributes
- `name` (label): configuration name.
- `schema` (string, optional): schema of the configuration.
- `parser` (string): text search parser.
- `mapping` (block, multiple, optional): add mappings. Each block requires:
  - `for` (array of strings): token types.
  - `with` (array of strings): dictionaries to use.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
text_search_configuration "simple_config" {
  parser = "default"
}
```
