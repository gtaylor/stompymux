---
title: "mux.text.markdown"
type: docs
linkTitle: "markdown"
manualLinkTitle: "markdown"
---

Parses Markdown into an immutable document for text output.

Raises `mux.error.codes.text.invalid` for invalid or oversized input.

## Signature

```lua
mux.text.markdown(source)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `source` | `string` | Markdown source. |

## Returns

- `MarkdownDocument document`

## Related errors

- `mux.error.codes.text.invalid`
