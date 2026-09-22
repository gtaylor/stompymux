---
title: "btech.unit.stand"
type: docs
linkTitle: "stand"
manualLinkTitle: "stand"
---

Attempt to stand, staging fall and terrain-break notices in the callback transaction.

## Signature

```lua
btech.unit.stand(dbref, pilot, mode)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `mode?` | `'normal'\|'anyway'\|'careful'` |  |

## Returns

- `table attempt Contains check, optional fall, rise/retry timer and ordered notices.`
