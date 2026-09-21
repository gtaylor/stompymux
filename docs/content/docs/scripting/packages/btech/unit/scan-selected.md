---
title: "btech.unit.scan_selected"
type: docs
---

Scan the saved unit or coordinate target without advancing its lock countdown.
Unit reports are returned; building/hex reports also publish cockpit output.
Selected coordinates allow observer distance exemptions while retaining visibility checks.

## Signature

```lua
btech.unit.scan_selected(dbref, pilot, options)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Scanner unit dbref. |
| `pilot` | `integer` |  |
| `options` | `string?` | A/I/W unit report sections. |

## Returns

- `BattleSelectedScan`
