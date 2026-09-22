---
title: "btech.unit.takeoff"
type: docs
linkTitle: "takeoff"
manualLinkTitle: "takeoff"
---

Queue VTOL takeoff using configured fuel rules and stage a cockpit confirmation.

## Signature

```lua
btech.unit.takeoff(dbref, pilot, delay)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `delay` | `integer?` | Extra launch seconds, 0..65535; nonzero requires a wizard. |

## Returns

- `boolean`
