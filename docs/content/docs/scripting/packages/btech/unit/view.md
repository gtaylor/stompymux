---
title: "btech.unit.view"
type: docs
linkTitle: "view"
manualLinkTitle: "view"
---

View escaped markings through running cockpit/gunner contact and unblocked-LOS admission.
No scan-range limit; omitted target uses this operator's selected unit.

## Signature

```lua
btech.unit.view(unit, actor, target)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `integer` | Cockpit or gunner station. |
| `actor` | `integer` |  |
| `target?` | `integer` |  |

## Returns

- `string text Styled report safe for normal output.`
