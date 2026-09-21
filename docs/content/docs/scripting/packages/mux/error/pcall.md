---
title: "mux.error.pcall"
type: docs
---

Calls a function, returning all results on success or a normalized traced error.

## Signature

```lua
mux.error.pcall(fn, ...)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `fn` | `fun(...):` | R... |
| `...` | `any` |  |

## Returns

- `true, R...`
