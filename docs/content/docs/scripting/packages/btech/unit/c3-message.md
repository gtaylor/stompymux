---
title: "btech.unit.c3_message"
type: docs
linkTitle: "c3_message"
manualLinkTitle: "c3_message"
---

Send to available classic C3 peers using current master capacity; echo to your cockpit.

## Signature

```lua
btech.unit.c3_message(dbref, pilot, message)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `message` | `string` |  |

## Returns

- `Notice[]|nil`
- `table|nil error`
