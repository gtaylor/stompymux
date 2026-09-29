---
title: "btech.unit.melee"
type: docs
linkTitle: "melee"
manualLinkTitle: "melee"
---

Swing the physical weapon installed in each selected arm (axe, sword, mace, dual saw, claw,
retractable blade, lance, flail, wrecking ball, chain whip or vibroblade). Both arms by
default, skipping arms without a weapon; only claws let the second arm follow a completed swing.

## Signature

```lua
btech.unit.melee(dbref, pilot, arms, target)
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
