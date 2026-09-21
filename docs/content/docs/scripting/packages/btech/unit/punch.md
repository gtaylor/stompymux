---
title: "btech.unit.punch"
type: docs
---

Attempt one or both arms in left-to-right order. Default selection is both.
Unavailable arms are reported separately when another arm attacks; an entirely rejected action raises an error.
Impact failures and callback aborts roll back the complete action and staged messages.

## Signature

```lua
btech.unit.punch(dbref, pilot, arms, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `arms?` | `'left'\|'right'\|'both'` |  |
| `target?` | `integer` | Defaults to the selected target; explicit targets require acquisition. |

## Returns

- `table report Ordered attacks, per-arm rejections and notices. Each attack includes its profile and optional impact.`
