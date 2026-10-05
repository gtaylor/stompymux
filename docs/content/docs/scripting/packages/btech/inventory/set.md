---
title: "btech.inventory.set"
type: docs
linkTitle: "set"
manualLinkTitle: "set"
---

Wizard stock correction using stored identifiers; zero quantity removes the entry.
Stock, immediate load correction and economy log records participate in callback rollback.
Unchanged quantities emit no record. Does not install equipment or perform cargo loading.

## Signature

```lua
btech.inventory.set(actor, object, part, quantity)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `object` | `integer` |  |
| `part` | `integer` | Nonnegative signed-32-bit identifier. |
| `quantity` | `integer` | From zero through 2147483647. |

## Returns

No values.
