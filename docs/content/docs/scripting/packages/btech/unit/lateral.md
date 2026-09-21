---
title: "btech.unit.lateral"
type: docs
---

Request a six-second lateral change; requires the assigned Maneuvering Ace pilot.

## Signature

```lua
btech.unit.lateral(dbref, player, direction)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `player` | `integer` |  |
| `direction` | `string` | nw/fl, ne/fr, sw/rl, se/rr, or - to travel straight. |

## Returns

- `BattleNotice`
