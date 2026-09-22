---
title: "btech.gunner.aim"
type: docs
linkTitle: "aim"
manualLinkTitle: "aim"
---

Preview conventional aim using independent station targeting and the registered gunner's skill.
Read-only; does not check launch authority, weapon readiness or station arc eligibility.

## Signature

```lua
btech.gunner.aim(station, gunner, weapon, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `station` | `integer` |  |
| `gunner` | `integer` |  |
| `weapon` | `integer` | Zero-based parent weapon index. |
| `target` | `integer\|table\|nil` | Unit ID, coordinates {x,y}, or the station's selection. |

## Returns

- `table report Parent, optional target/coordinate, and shared aim breakdown.`
