---
title: "btech.unit.scan_hex"
type: docs
linkTitle: "scan_hex"
manualLinkTitle: "scan_hex"
---

Scan the first acquired visible occupant at a coordinate in saved map order.
Uses the unit-scan report and warning path; empty and unacquired hexes share one reply.

## Signature

```lua
btech.unit.scan_hex(dbref, pilot, x, y, options)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Scanner unit dbref. |
| `pilot` | `integer` |  |
| `x` | `integer` | Map column. |
| `y` | `integer` | Map row. |
| `options` | `string?` | A/I/W sections; omitted means all. |

## Returns

- `string Styled report or empty-hex reply.`
