---
title: "btech.unit.radio_frequency"
type: docs
linkTitle: "radio_frequency"
manualLinkTitle: "radio_frequency"
---

Set a channel frequency without transmitting. Transactional; assigned conscious pilot required.

## Signature

```lua
btech.unit.radio_frequency(dbref, pilot, channel, frequency)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `channel` | `integer` | Zero-based channel (A is 0). |
| `frequency` | `integer` | From 0 through 999999. |

## Returns

- `boolean`
