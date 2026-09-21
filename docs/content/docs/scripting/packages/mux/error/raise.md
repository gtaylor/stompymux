---
title: "mux.error.raise"
type: docs
---

Raises a structured error with the requested code.

Raises the requested code. Ordinary Lua type errors are raised when `code`
or `message` cannot be converted to a string.

## Signature

```lua
mux.error.raise(code, message, detail)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `code` | `string\|ErrorCode` |  |
| `message` | `string` |  |
| `detail?` | `any` |  |

## Returns

No values.
