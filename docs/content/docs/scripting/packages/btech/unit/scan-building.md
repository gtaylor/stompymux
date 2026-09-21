---
title: "btech.unit.scan_building"
type: docs
---

Scan a structure entrance and publish its integrity report to cockpit occupants.
Hidden structures require an active in-character perception roll; invisible ones stay undetected.
Dice, experience and output commit together. Explicit coordinates retain observer range limits.

## Signature

```lua
btech.unit.scan_building(dbref, pilot, x, y)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Scanner unit dbref. |
| `pilot` | `integer` |  |
| `x` | `integer` | Map column. |
| `y` | `integer` | Map row. |

## Returns

- `BattleBuildingScan`
