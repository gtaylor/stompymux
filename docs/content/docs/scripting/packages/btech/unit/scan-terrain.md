---
title: "btech.unit.scan_terrain"
type: docs
---

Scan buildings then mines in one transaction and publish both phases.
Failed mine recognition is private to the pilot; success reaches cockpit occupants.

## Signature

```lua
btech.unit.scan_terrain(dbref, pilot, x, y)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Scanner unit dbref. |
| `pilot` | `integer` |  |
| `x` | `integer` |  |
| `y` | `integer` |  |

## Returns

- `BattleHexScan`
