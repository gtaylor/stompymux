---
title: "btech.unit.speed"
type: docs
linkTitle: "speed"
manualLinkTitle: "speed"
---

Set desired speed within the running unit's forward/reverse limits and stage a cockpit confirmation.

## Signature

```lua
btech.unit.speed(dbref, player, kph)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `player` | `integer` |  |
| `kph` | `number\|string?` | Omit to read actual speed; names include walk/cruise, run/flank, stop and back. Numeric requests clamp to throttle limits. |

## Returns

- `boolean|number`
