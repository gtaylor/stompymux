---
title: "btech.unit.tactical"
type: docs
linkTitle: "tactical"
manualLinkTitle: "tactical"
---

Render standard, C/T (mech/tank cliffs), B (landing zones), M (mines), L (visible), or U (underlying) tactical maps.
Uses shared cockpit/display admission; no acquisition rolls, notices or state changes.

## Signature

```lua
btech.unit.tactical(dbref, pilot, arguments)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Scanner unit dbref. |
| `pilot` | `integer` |  |
| `arguments` | `string?` | Optional C/T/B/M/L/U flag followed by shared centering arguments. |

## Returns

- `BattleTacticalMap`
