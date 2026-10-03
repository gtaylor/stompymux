---
title: "btech.inventory.forms"
type: docs
linkTitle: "forms"
manualLinkTitle: "forms"
---

Return all part forms in short-name order, without requiring live stock.

## Signature

```lua
btech.inventory.forms(actor)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` | Wizard requesting inspection. |

## Returns

- `table[] forms Part ID, short_name, long_name and very_long_name.`
