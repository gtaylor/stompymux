---
title: "btech.autopilot.cancel"
type: docs
linkTitle: "cancel"
manualLinkTitle: "cancel"
---

Cancel an active or queued order.

## Signature

```lua
btech.autopilot.cancel(unit, order_id, expected_revision)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `integer` | The controlled unit. |
| `order_id` | `integer` | The stable order ID. |
| `expected_revision` | `integer` | Optional revision guard. |

## Returns

- `boolean True when an order was canceled.`
