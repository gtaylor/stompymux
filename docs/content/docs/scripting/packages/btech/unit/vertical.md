---
title: "btech.unit.vertical"
type: docs
---

Read or set VTOL vertical speed using configured fuel rules and the shared velocity budget.

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
