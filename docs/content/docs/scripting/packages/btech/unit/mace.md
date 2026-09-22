---
title: "btech.unit.mace"
type: docs
linkTitle: "mace"
manualLinkTitle: "mace"
---

Attempt a mace swing. A missed swing requires an attacker piloting check with a +2 modifier.

## Signature

```lua
btech.unit.mace(dbref, pilot, arms, target)
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
