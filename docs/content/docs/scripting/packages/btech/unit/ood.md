---
title: "btech.unit.ood"
type: docs
---

Wizard orbital insertion. Detach towing first; reject prone units and active digging.
Ground chassis receive mass-based cocoons. VTOLs enter flight with half-speed requested.
Stopped VTOLs retain the inserted pose and controls until startup finishes.
An attached on_ood_land event runs before landing dice/damage, with the arriving unit as object/enactor/cause.
Callback errors restore the entire airborne tick and its output.

## Signature

```lua
btech.unit.ood(actor, unit, x, y, z)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` | Wizard actor and confirmation recipient. |
| `unit` | `integer` | Placed physical unit. |
| `x` | `integer` |  |
| `y` | `integer` |  |
| `z?` | `integer` | Signed-short altitude; nil defaults to 300. |

## Returns

- `{position: table, elevation: integer, drop: table|nil, flight: table|nil} report`
