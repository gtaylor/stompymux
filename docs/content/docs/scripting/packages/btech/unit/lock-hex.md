---
title: "btech.unit.lock_hex"
type: docs
linkTitle: "lock_hex"
manualLinkTitle: "lock_hex"
---

Select valid map coordinates without requiring visibility. Unit-at-hex fire uses its current occupant; empty-hex and terrain attacks remain unimplemented.

## Signature

```lua
btech.unit.lock_hex(dbref, pilot, x, y, mode)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `x` | `integer` |  |
| `y` | `integer` |  |
| `mode?` | `string` | H/hex, B/building, I/ignite, C/clear; omitted means unit at hex. |

## Returns

- `boolean`
