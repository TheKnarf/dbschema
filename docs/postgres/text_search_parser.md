# Text Search Parser

Defines a full text search parser.

```hcl
text_search_parser "my_parser" {
  start    = "ts_parse_start"
  gettoken = "ts_parse_gettoken"
  end      = "ts_parse_end"
  lextypes = "ts_parse_lextypes"
  headline = "ts_parse_headline"
}
```

## Attributes
- `name` (label): parser name.
- `schema` (string, optional): schema of the parser.
- `start` (string): start function.
- `gettoken` (string): gettoken function.
- `end` (string): end function.
- `lextypes` (string): lextypes function.
- `headline` (string, optional): headline function.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
text_search_parser "simple_parser" {
  start    = "ts_parse_start"
  gettoken = "ts_parse_gettoken"
  end      = "ts_parse_end"
  lextypes = "ts_parse_lextypes"
}
```
