---
title: "btech.cargo.unload"
type: docs
linkTitle: "unload"
manualLinkTitle: "unload"
---

Unload matching CargoTech stock onto the current map; startup and loading-point checks do not apply.

## Signature

```lua
btech.cargo.unload(actor, pattern, quantity)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `pattern` | `string` |  |
| `quantity` | `integer` | Positive request per matched row, capped at 50000 and available stock. |

## Returns

- `BattleCargoRow[] Transferred quantities.`
