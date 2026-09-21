---
title: "mux.text.truncate"
type: docs
---

Safely truncates styled text to a non-negative visible byte width.

Raises `mux.error.codes.text.invalid`.

## Signature

```lua
mux.text.truncate(value, width)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `value` | `string` |  |
| `width` | `integer` |  |

## Returns

- `string truncated`

## Related errors

- `mux.error.codes.text.invalid`
