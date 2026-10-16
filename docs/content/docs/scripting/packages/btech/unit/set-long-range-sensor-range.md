---
title: "btech.unit.set_long_range_sensor_range"
type: docs
linkTitle: "set_long_range_sensor_range"
manualLinkTitle: "set_long_range_sensor_range"
---

Set the unit's long-range display reach in hexes (0 to 127), replacing the 50-hex default.
The reach applies as given; only sensor criticals taken afterwards halve or remove it, and 0
turns the display off. Read it back from `btech.unit.state(unit).sensor_ranges.long_range`.

## Signature

```lua
btech.unit.set_long_range_sensor_range(unit, range)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `DbRef\|Object` |  |
| `range` | `integer` |  |

## Returns

No values.
