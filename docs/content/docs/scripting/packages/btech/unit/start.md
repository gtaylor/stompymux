---
title: "btech.unit.start"
type: docs
linkTitle: "start"
manualLinkTitle: "start"
---

Start the assigned pilot's unit. Trusted scripts authorize the optional fast override.

## Signature

```lua
btech.unit.start(dbref, player, fast)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `player` | `integer` |  |
| `fast` | `boolean\|nil` | Five-second override; otherwise 30 seconds. |

## Returns

- `boolean`
