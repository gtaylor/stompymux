---
title: "btech.gunner.fire"
type: docs
linkTitle: "fire"
manualLinkTitle: "fire"
---

Fire a parent weapon at a unit or coordinate using the registered station operator and independent selection.
Uses shared launch/damage and transactional publication, including delayed artillery and station-owned correction.

## Signature

```lua
btech.gunner.fire(station, gunner, weapon, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `station` | `integer` |  |
| `gunner` | `integer` |  |
| `weapon` | `integer` | Zero-based parent weapon index. |
| `target` | `integer\|table\|nil` | Unit ID, coordinates {x,y}, or station selection. |

## Returns

- `table report Shared physical-unit firing report.`
