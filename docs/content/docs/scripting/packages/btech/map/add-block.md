---
title: "btech.map.add_block"
type: docs
---

Add a wizard-owned circular landing restriction; negative radii block nothing.

## Signature

```lua
btech.map.add_block(actor, dbref, x, y, radius, team)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `dbref` | `integer` |  |
| `x` | `integer` |  |
| `y` | `integer` |  |
| `radius` | `integer` |  |
| `team?` | `integer` | Zero means no exemption. |

## Returns

- `integer Restriction slot.`
