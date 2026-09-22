---
title: "btech.unit.jump"
type: docs
linkTitle: "jump"
manualLinkTitle: "jump"
---

Attempt a jump; a failed stagger check falls instead of launching, within the callback transaction.
A completed check survives later destination rejection; an enclosing callback failure still rolls back.

## Signature

```lua
btech.unit.jump(dbref, pilot, bearing, range)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` | Conscious assigned pilot; scripts own authority to act for them. |
| `bearing` | `integer` | Compass degrees. |
| `range` | `number` | Positive range in hex heights, snapped to a destination hex center. |

## Returns

- `boolean Accepted attempt; inspect flight state to distinguish launch from a stagger fall.`
