---
title: "btech.unit.view"
type: docs
linkTitle: "view"
manualLinkTitle: "view"
---

View escaped markings through running cockpit contact and unblocked-LOS admission.
No scan-range limit; omitted target uses this operator's selected unit.

## Signature

```lua
btech.unit.view(unit, actor, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `integer` | Cockpit unit. |
| `actor` | `integer` |  |
| `target?` | `integer` |  |

## Returns

- `string text Styled report safe for normal output.`
