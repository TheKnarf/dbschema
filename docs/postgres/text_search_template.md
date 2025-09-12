# Text Search Template

Defines a full text search template.

```hcl
text_search_template "my_template" {
  lexize = "simple_lexize"
  init   = "simple_init"
}
```

## Attributes
- `name` (label): template name.
- `schema` (string, optional): schema of the template.
- `lexize` (string): lexize function.
- `init` (string, optional): init function.
- `comment` (string, optional): documentation comment.

## Examples

```hcl
text_search_template "simple_template" {
  lexize = "simple_lexize"
}
```
