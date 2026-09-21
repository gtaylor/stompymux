---
title: "btech.unit.radio_mode"
type: docs
---

Replace channel mode: D digital, U muted, E relay; optional color letter. Transactional.
Relay requires digital mode and capable hardware. Empty selects analog with no flags.

## Signature

```lua
btech.unit.radio_mode(dbref, pilot, channel, mode)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `channel` | `integer` | Zero-based channel (A is 0). |
| `mode` | `string` |  |

## Returns

- `boolean`
