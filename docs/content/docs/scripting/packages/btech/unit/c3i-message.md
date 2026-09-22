---
title: "btech.unit.c3i_message"
type: docs
linkTitle: "c3i_message"
manualLinkTitle: "c3i_message"
---

Send text to available C3i peers and echo it to your cockpit. Requires an active transaction.

## Signature

```lua
btech.unit.c3i_message(dbref, pilot, message)
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
