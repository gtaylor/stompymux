---
title: "btech.cargo.load"
type: docs
linkTitle: "load"
manualLinkTitle: "load"
---

Load matching hangar stock into a stationary, running CargoTech unit.
Exact abbreviations precede exact catalogue names, then wildcard names; selection is independent of available stock.
Transfers, throttle correction and EconInfo diagnostics are atomic and participate in callback rollback.

## Signature

```lua
btech.cargo.load(actor, pattern, quantity)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `pattern` | `string` |  |
| `quantity` | `integer` | Positive request per matched row, capped at 50000 and available stock. |

## Returns

- `CargoRow[] Transferred quantities.`
