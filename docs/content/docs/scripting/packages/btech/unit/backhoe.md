---
title: "btech.unit.backhoe"
type: docs
linkTitle: "backhoe"
manualLinkTitle: "backhoe"
---

Dig a vehicle's backhoe into the adjacent building hex at `x`, `y`, or the hex its coordinate
lock names. The blow always lands, for six points plus 1D6 against the building's construction
factor, and may bring it down; a vehicle's backhoe never attacks a unit.

## Signature

```lua
btech.unit.backhoe(dbref, pilot, x, y)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `x?` | `integer` | Both coordinates or neither. |
| `y?` | `integer` |  |

## Returns

- `table report Roll, damage, construction factor left, any collapse and notices.`
