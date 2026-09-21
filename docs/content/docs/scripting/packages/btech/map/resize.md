---
title: "btech.map.resize"
type: docs
---

Wizard-only resize; copies overlapping visible tiles, clears map objects and rejects clipped units.

## Signature

```lua
btech.map.resize(actor, dbref, width, height)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `dbref` | `integer` |  |
| `width` | `integer` | 1 through 1000 |
| `height` | `integer` | 1 through 1000 |

## Returns

- `boolean`
