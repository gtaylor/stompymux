---
title: "btech.unit.sight"
type: docs
linkTitle: "sight"
manualLinkTitle: "sight"
---

Sight a weapon using ordinary target selection and aim, without firing or revealing cover.
Consumes preparation and attack dice; ignores ammunition, recycling and feed jams.
Requires an intact offensive mount and the conscious assigned pilot of a running unit.

## Signature

```lua
btech.unit.sight(dbref, pilot, weapon, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `weapon` | `integer` | Zero-based weapon index. |
| `target` | `integer\|BattleHexCoordinate\|nil` | Omitted target uses cockpit selection. |

## Returns

- `BattleSightReport`
