---
title: "btech.inventory.fix"
type: docs
---

Wizard cleanup of loose stock, removing structural placeholders and unknown identifiers.
Preserves installed equipment; reconciles carrying load. Callback failure restores inventory.

## Signature

```lua
btech.inventory.fix(actor, object)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `object` | `integer` |  |

## Returns

- `BattleInventoryCleanup`
