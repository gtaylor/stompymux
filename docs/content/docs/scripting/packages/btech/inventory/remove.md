---
title: "btech.inventory.remove"
type: docs
---

Wizard removal floors stock at zero; reports and diagnostics retain the capped requested amount.

## Signature

```lua
btech.inventory.remove(actor, object, pattern, quantity)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `object` | `integer` |  |
| `pattern` | `string` |  |
| `quantity` | `integer` | Positive requested quantity per match. |

## Returns

- `BattleCargoRow[]`
