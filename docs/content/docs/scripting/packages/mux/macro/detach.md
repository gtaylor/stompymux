---
title: "mux.macro.detach"
type: docs
linkTitle: "detach"
manualLinkTitle: "detach"
---

Clear a slot and its editing selection, if selected.
Requires an active callback transaction; bypasses player permission checks.

## Signature

```lua
mux.macro.detach(player, slot)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `player` | `DbRef\|Object` |  |
| `slot` | `integer` | Zero-based slot, 0 through 4. |

## Returns

- `boolean removed`
