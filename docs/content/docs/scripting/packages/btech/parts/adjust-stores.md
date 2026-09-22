---
title: "btech.parts.adjust_stores"
type: docs
linkTitle: "adjust_stores"
manualLinkTitle: "adjust_stores"
---

Apply one signed atomic stock edit; a nonzero integral delta is required.

## Signature

```lua
btech.parts.adjust_stores(target, part, delta)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `target` | `DbRef\|Object` | Live object holding stock. |
| `part` | `BattlePartRef` |  |
| `delta` | `integer` |  |

## Returns

No values.
