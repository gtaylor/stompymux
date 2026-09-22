---
title: "btech.map.set_hex"
type: docs
linkTitle: "set_hex"
manualLinkTitle: "set_hex"
---

Wizard live base-terrain edit. Retains unit altitude and overlays; does not cause combat falls.

## Signature

```lua
btech.map.set_hex(actor, dbref, x, y, terrain, elevation)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `dbref` | `integer` |  |
| `x` | `integer` |  |
| `y` | `integer` |  |
| `terrain` | `string` | Canonical terrain symbol; a leading dot selects grassland. |
| `elevation` | `integer` | Absolute magnitude capped at nine. |

## Returns

- `BattleMapHexChange`
