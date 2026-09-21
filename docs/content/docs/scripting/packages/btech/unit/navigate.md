---
title: "btech.unit.navigate"
type: docs
---

Show the radius-two local map and units within the selected center hex.

## Signature

```lua
btech.unit.navigate(dbref, pilot, arguments)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` | Conscious assigned pilot of a running unit. |
| `arguments` | `string?` | Own unit, contact label/dbref, or bearing and distance. |

## Returns

- `BattleNavigationReport`
