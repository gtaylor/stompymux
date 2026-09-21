---
title: "btech.unit.radio_target"
type: docs
---

Send to an acquired visible target; the source must be running and not an observer.
A shutdown target receives no message. This does not use channel frequencies or detonate mines.

## Signature

```lua
btech.unit.radio_target(dbref, pilot, target, message)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `target` | `integer` | Recipient dbref. |
| `message` | `string` | Nonempty text without control characters. |

## Returns

- `BattleTargetedRadioReport`
