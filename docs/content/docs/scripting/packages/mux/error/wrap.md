---
title: "mux.error.wrap"
type: docs
---

Wraps a failure as the cause of a new structured error.

Non-error causes are normalized to
`mux.error.codes.runtime`. Ordinary Lua type
errors are raised when `code` or `message` cannot be converted to a string.

## Signature

```lua
mux.error.wrap(err, code, message)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `err` | `any` |  |
| `code` | `string\|ErrorCode` |  |
| `message` | `string` |  |

## Returns

- `Error error`

## Related errors

- `mux.error.codes.runtime`
