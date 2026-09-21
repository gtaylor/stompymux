---
title: "btech.unit.pickup"
type: docs
---

Pick up a visible unit using shared towing, shutdown and terrain rules.

## Signature

```lua
btech.unit.pickup(dbref, pilot, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Carrier unit. |
| `pilot` | `integer` | Conscious assigned pilot; scripts own authority to act for them. |
| `target` | `integer` | Target unit. |

## Returns

- `boolean`
