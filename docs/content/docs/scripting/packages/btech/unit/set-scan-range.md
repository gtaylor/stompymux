---
title: "btech.unit.set_scan_range"
type: docs
linkTitle: "set_scan_range"
manualLinkTitle: "set_scan_range"
---

Set the unit's `scan` reach in hexes (0 to 127), replacing the 25-hex default.
The reach applies as given; only sensor criticals taken afterwards halve or remove it, and 0
turns scanning off. Read it back from `btech.unit.state(unit).sensor_ranges.scan`.

## Signature

```lua
btech.unit.set_scan_range(unit, range)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `DbRef\|Object` |  |
| `range` | `integer` |  |

## Returns

No values.
