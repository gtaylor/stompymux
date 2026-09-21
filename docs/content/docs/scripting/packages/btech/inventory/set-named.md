---
title: "btech.inventory.set_named"
type: docs
---

Wizard stock correction by exact part name, sharing validation and callback rollback with set.

## Signature

```lua
btech.inventory.set_named(actor, object, name, brand, quantity)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `object` | `integer` |  |
| `name` | `string` |  |
| `brand` | `integer` | From zero through five. |
| `quantity` | `integer` | From zero through 2147483647. |

## Returns

No values.
