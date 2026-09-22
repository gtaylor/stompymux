---
title: "btech.unit.saw"
type: docs
linkTitle: "saw"
manualLinkTitle: "saw"
---

Attempt a dual-saw attack; seven operational parts required, fixed seven base damage without TSM boost.

## Signature

```lua
btech.unit.saw(dbref, pilot, arms, target)
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
