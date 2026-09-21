---
title: "btech.unit.radio_send"
type: docs
---

Transmit using the selected channel; delivery and command mines commit together.
Requires a conscious assigned cockpit pilot and no stun. Shutdown radios remain usable.

## Signature

```lua
btech.unit.radio_send(dbref, pilot, channel, message)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `channel` | `integer` | Zero-based channel (A is 0). |
| `message` | `string` | Nonempty text without control characters. |

## Returns

- `BattleRadioTransmission`
