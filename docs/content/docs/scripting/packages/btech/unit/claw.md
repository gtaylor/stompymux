---
title: "btech.unit.claw"
type: docs
linkTitle: "claw"
manualLinkTitle: "claw"
---

Attempt claw attacks, left then right by default; each accepted arm starts its own recovery.

## Signature

```lua
btech.unit.claw(dbref, pilot, arms, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `arms?` | `'left'\|'right'\|'both'` |  |
| `target?` | `integer` |  |

## Returns

- `table report Ordered attacks, per-arm rejections and transactional notices.`
