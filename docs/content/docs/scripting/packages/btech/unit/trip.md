---
title: "btech.unit.trip"
type: docs
---

Attempt a leg trip. A hit forces target balance; a miss has no balance check. No direct impact damage.
Both legs and hips must be usable; the target must be standing and not rising.

## Signature

```lua
btech.unit.trip(dbref, pilot, leg, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `leg?` | `'left'\|'right'` | Defaults to right. |
| `target?` | `integer` | Defaults to selected target; explicit targets require acquisition. |

## Returns

- `table report Attack profile, roll, hit, glancing, optional balance/fall, and notices.`
