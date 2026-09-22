---
title: "btech.unit.setxy"
type: docs
linkTitle: "setxy"
manualLinkTitle: "setxy"
---

Wizard repositioning within the current battlefield, preserving controls and tow attachment.

## Signature

```lua
btech.unit.setxy(actor, unit, x, y, z)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` | Wizard actor and confirmation recipient. |
| `unit` | `integer` | Placed physical unit. |
| `x` | `integer` |  |
| `y` | `integer` |  |
| `z?` | `integer` | Signed-short altitude; nil selects the surface and lands VTOLs. |

## Returns

- `{position: table, elevation: integer} report`
