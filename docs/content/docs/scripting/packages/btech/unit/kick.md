---
title: "btech.unit.kick"
type: docs
---

Attempt a biped kick; rolls back damage, falls, recovery and notices with the callback.

## Signature

```lua
btech.unit.kick(dbref, pilot, leg, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `leg?` | `'left'\|'right'` | Defaults to right. |
| `target?` | `integer` | Defaults to the selected target; explicit targets require acquisition. |

## Returns

- `table report Attack profile, roll, hit, glancing, impact, balance, fall and notices.`
