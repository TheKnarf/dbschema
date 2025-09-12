# Text Search Dictionary

Defines a full text search dictionary.

```hcl
text_search_dictionary "english" {
  schema   = "public"
  template = "simple"
  options  = ["dictfile = 'english'"]
}
```

## Attributes
- `name` (label): dictionary name.
- `schema` (string, optional): schema of the dictionary.
- `template` (string): template to use.
- `options` (array of strings, optional): additional `OPTION` entries.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
text_search_dictionary "simple_dict" {
  template = "simple"
}
```
