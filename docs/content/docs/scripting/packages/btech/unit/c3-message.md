---
title: "btech.unit.c3_message"
type: docs
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

- `BattleNotice[]|nil`
- `table|nil error`
