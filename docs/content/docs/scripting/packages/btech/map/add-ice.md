---
title: "btech.map.add_ice"
type: docs
linkTitle: "add_ice"
manualLinkTitle: "add_ice"
---

Wizard seasonal growth. Only water can freeze; new ice does not extend this pass's shoreline.

## Signature

```lua
btech.map.add_ice(actor, dbref, percentage)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `dbref` | `integer` |  |
| `percentage` | `integer` | Signed percentage threshold; outside 0–100 means never/always. |

## Returns

- `BattleMapIceReport`
