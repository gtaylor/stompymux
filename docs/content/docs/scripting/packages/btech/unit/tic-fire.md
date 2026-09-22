---
title: "btech.unit.tic_fire"
type: docs
linkTitle: "tic_fire"
manualLinkTitle: "tic_fire"
---

Fire groups in ascending order using ordinary firing rules. Shot rejection continues;
fall or shutdown stops firing. Callback failure restores the whole batch.

## Signature

```lua
btech.unit.tic_fire(dbref, pilot, groups, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `groups` | `integer[]` | Zero-based group numbers, 0 through 3. |
| `target` | `integer\|{x: integer, y: integer}\|nil` | Explicit unit/coordinates, or cockpit selection when omitted. |

## Returns

- `{group: integer, weapon: integer, report: table|nil, rejection: string|nil}[]`
