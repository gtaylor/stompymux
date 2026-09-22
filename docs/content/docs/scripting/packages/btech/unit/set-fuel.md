---
title: "btech.unit.set_fuel"
type: docs
linkTitle: "set_fuel"
manualLinkTitle: "set_fuel"
---

Wizard fuel correction bounded by current capacity and 4294967295.
Fuel and throttle changes participate in callback rollback; this does not repair lost lift.

## Signature

```lua
btech.unit.set_fuel(actor, unit, amount)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `unit` | `integer` |  |
| `amount` | `integer` | Nonnegative remaining fuel. |

## Returns

- `BattleVtolFuelStatus`
