---
title: "btech.inventory.add_stores"
type: docs
linkTitle: "add_stores"
manualLinkTitle: "add_stores"
---

Wizard signed adjustment of one catalogue match, ordered by long name after exact matching.
Positive counts cap at 50000; negative counts are not capped. Zero succeeds without matching.
Stock, load correction and diagnostics roll back together. No match returns false.

## Signature

```lua
btech.inventory.add_stores(actor, object, pattern, quantity)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `object` | `integer` |  |
| `pattern` | `string` | Fewer than 2048 bytes. |
| `quantity` | `integer` | Signed 32-bit count. |

## Returns

- `boolean`
