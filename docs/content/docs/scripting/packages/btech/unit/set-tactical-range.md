---
title: "btech.unit.set_tactical_range"
type: docs
linkTitle: "set_tactical_range"
manualLinkTitle: "set_tactical_range"
---

Set the unit's tactical display reach in hexes (0 to 127), replacing the 25-hex default.
The reach applies as given; only sensor criticals taken afterwards halve or remove it, and 0
turns the display off. Read it back from `btech.unit.state(unit).sensor_ranges.tactical`.

## Signature

```lua
btech.unit.set_tactical_range(unit, range)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `DbRef\|Object` |  |
| `range` | `integer` |  |

## Returns

No values.
