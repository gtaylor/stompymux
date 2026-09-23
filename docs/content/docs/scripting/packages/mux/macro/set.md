---
title: "mux.macro.set"
type: docs
linkTitle: "set"
manualLinkTitle: "set"
---

Resolve a transient set number; absent sets return nil.
Requires an active callback transaction; bypasses player permission checks.

## Signature

```lua
mux.macro.set(number)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `number` | `integer` | Zero-based current set number. |

## Returns

- `MacroSet? set`
