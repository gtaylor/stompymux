---
title: "btech.unit.disable"
type: docs
---

Power down a Gauss mount after recharge. Requires a running, mapped unit and conscious pilot.
Persists across shutdown/restart, prevents firing and suppresses Gauss critical explosions.

## Signature

```lua
btech.unit.disable(dbref, pilot, weapon)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `weapon` | `integer` | Zero-based weapon number. |

## Returns

- `boolean`
