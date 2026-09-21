---
title: "btech.unit.axe"
type: docs
---

Attempt an axe swing. Default selection tries equipped arms left first; an accepted swing blocks the other arm through recovery.

## Signature

```lua
btech.unit.axe(dbref, pilot, arms, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `arms?` | `'left'\|'right'\|'both'` |  |
| `target?` | `integer` | Defaults to selected target; explicit targets require acquisition. |

## Returns

- `table report Ordered attacks, per-arm rejections and transactional notices.`
