---
title: "btech.unit.setmapindex"
type: docs
---

Wizard map assignment; -1 removes membership and retains the pose for re-entry.
A removed running unit shuts down on the next simulation update.

## Signature

```lua
btech.unit.setmapindex(actor, unit, map, preferred)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` | Wizard and private confirmation recipient. |
| `unit` | `integer` | Physical constructed unit. |
| `map` | `integer` | Decimal map dbref, or -1 for removal. |
| `preferred?` | `string` | First two bytes override configuration; short/nil values use the saved preference, then random selection. |

## Returns

- `{assignment: table|nil} report Assigned position, label and reset_origin, or nil on removal.`
