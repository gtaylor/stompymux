---
title: "btech.tactical.observe"
type: docs
linkTitle: "observe"
manualLinkTitle: "observe"
---

Read a detached tactical snapshot for 1 to 100 distinct friendly controllers on one map.
Shared sightings retain observer provenance and do not grant another unit attack admission.

## Signature

```lua
btech.tactical.observe(units, feedback_cursors)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `units` | `integer[]` | Explicit assigned unit IDs; every unit must be attached and placed. |
| `feedback_cursors` | `table<integer, integer>\|nil` | Optional per-unit feedback sequence cursors. |

## Returns

- `BattleTacticalSnapshot snapshot Versioned intelligence and controller outcomes.`
