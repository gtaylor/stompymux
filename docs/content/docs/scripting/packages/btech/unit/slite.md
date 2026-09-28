---
title: "btech.unit.slite"
type: docs
linkTitle: "slite"
manualLinkTitle: "slite"
---

Without a mode, schedule a five-second manual toggle; repeated calls preserve the pending switch.
With a mode, select it and steer the lamp toward it. AUTO lights the lamp at night and
extinguishes it otherwise, re-evaluated when map light changes, the unit changes maps or
finishes starting up.

## Signature

```lua
btech.unit.slite(dbref, pilot, mode)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `mode?` | `BattleSearchlightMode` | Typed constant from btech.unit.searchlight_modes. |

## Returns

- `boolean`
