---
title: "btech.map.set_hex"
type: docs
linkTitle: "set_hex"
manualLinkTitle: "set_hex"
---

Wizard live base-terrain edit replacing one hex's layers, in the shape btech.map.hex returns. Retains unit altitude and fire and smoke; does not cause combat falls. A hex with an overlay is rejected; use add_fire and add_smoke.

## Signature

```lua
btech.map.set_hex(actor, dbref, x, y, hex)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `dbref` | `integer` |  |
| `x` | `integer` |  |
| `y` | `integer` |  |
| `hex` | `BattleHex` |  |

## Returns

- `BattleMapHexChange`
