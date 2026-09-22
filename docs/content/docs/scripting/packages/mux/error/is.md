---
title: "mux.error.is"
type: docs
linkTitle: "is"
manualLinkTitle: "is"
---

Tests a table's code using exact or dotted-prefix matching.

Raises an ordinary Lua type error when `code` cannot be converted to a string.

## Signature

```lua
mux.error.is(value, code)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `value` | `any` |  |
| `code` | `string\|ErrorCode` |  |

## Returns

- `boolean matches`
