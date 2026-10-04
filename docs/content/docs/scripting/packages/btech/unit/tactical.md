---
title: "btech.unit.tactical"
type: docs
linkTitle: "tactical"
manualLinkTitle: "tactical"
---

Render standard, C/T (mech/tank cliffs), B (landing zones), M (mines) or L (visible) tactical maps. Fire and smoke fill the top of a hex over the terrain beneath.
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
| `arguments` | `string?` | Optional C/T/B/M/L flag followed by shared centering arguments. |

## Returns

- `TacticalMap`
