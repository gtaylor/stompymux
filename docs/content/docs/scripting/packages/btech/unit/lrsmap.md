---
title: "btech.unit.lrsmap"
type: docs
---

Render long-range terrain, elevation or currently visible acquired units.
Mode initials match native LRS; descriptive API mode names are also accepted.
Dark maps mask unseen terrain. Rendering consumes no dice and sends no notices.

## Signature

```lua
btech.unit.lrsmap(dbref, pilot, mode, arguments)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Scanner unit dbref. |
| `pilot` | `integer` |  |
| `mode` | `string` | First letter T/E/C/M/L/H/S (case insensitive), or a descriptive API mode name. |
| `arguments` | `string?` | Shared centering arguments. |

## Returns

- `BattleLongRangeMap`
