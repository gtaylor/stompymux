---
title: "btech.gunner.sight"
type: docs
linkTitle: "sight"
manualLinkTitle: "sight"
---

Sight a parent weapon using the registered station gunner's selection, skill and arcs.
Supports conventional, terrain and artillery aim; consumes only preparation/attack dice.
Requires running controls but permits weapons hold, recycling and empty ammunition.

## Signature

```lua
btech.gunner.sight(station, gunner, weapon, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `station` | `integer` |  |
| `gunner` | `integer` |  |
| `weapon` | `integer` | Zero-based parent weapon index. |
| `target` | `integer\|BattleHexCoordinate\|nil` | Omitted target uses station selection and parent observer link. |

## Returns

- `BattleSightReport`
