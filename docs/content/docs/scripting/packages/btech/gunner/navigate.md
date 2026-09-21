---
title: "btech.gunner.navigate"
type: docs
---

Render local parent navigation, retaining the own-hex exception for failed scanner hardware.

## Signature

```lua
btech.gunner.navigate(station, gunner, arguments)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `station` | `integer` |  |
| `gunner` | `integer` |  |
| `arguments` | `string?` | Contact or bearing/range center; omitted center follows the parent. |

## Returns

- `BattleNavigationReport`
