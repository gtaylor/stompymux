---
title: "btech.inventory.add"
type: docs
linkTitle: "add"
manualLinkTitle: "add"
---

Wizard catalogue-based addition; amount is capped at 50000 per match.
Wizards other than GOD may select at most 20 catalogue entries. Stock and diagnostics commit together.

## Signature

```lua
btech.inventory.add(actor, object, pattern, quantity)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `object` | `integer` |  |
| `pattern` | `string` | Exact abbreviation/full name, then wildcard; may match absent stock. |
| `quantity` | `integer` | Positive requested quantity per match. |

## Returns

- `CargoRow[] Requested changes after the request cap.`
