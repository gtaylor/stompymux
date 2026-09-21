---
title: "btech.gunner.lrsmap"
type: docs
---

Render the parent's long-range map using the registered gunner's display preferences.

## Signature

```lua
btech.gunner.lrsmap(station, gunner, mode, arguments)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `station` | `integer` |  |
| `gunner` | `integer` |  |
| `mode` | `string` | Terrain/elevation/unit/visibility mode shared with unit.lrsmap. |
| `arguments` | `string?` | Contact or bearing/range center; omitted center follows the parent. |

## Returns

- `BattleLongRangeMap`
