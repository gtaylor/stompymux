---
title: "btech.unit.vertical"
type: docs
linkTitle: "vertical"
manualLinkTitle: "vertical"
---

Read or set VTOL vertical speed; climb and horizontal travel share one movement budget.

## Signature

```lua
btech.unit.vertical(dbref, pilot, kph)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `kph` | `number?` | Omit for current speed; positive climbs, negative descends. |

## Returns

- `boolean|number`
