---
title: "btech.autopilot.submit"
type: docs
linkTitle: "submit"
manualLinkTitle: "submit"
---

Validate and queue typed movement or combat orders.

## Signature

```lua
btech.autopilot.submit(unit, orders, mode, expected_revision)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `integer` | The controlled unit. |
| `orders` | `AutopilotOrder[]` | Order specification tables. |
| `mode` | `AutopilotSubmissionMode` | Append or replace the existing queue. |
| `expected_revision` | `integer` | Optional revision guard. |

## Returns

- `AutopilotSubmitResult Order IDs and the new management revision.`
