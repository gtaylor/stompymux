---
title: "btech.unit.c3i_network"
type: docs
---

Inspect running, unjammed peers without requiring visual contact or publishing output.

## Signature

```lua
btech.unit.c3i_network(dbref, pilot)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |

## Returns

- `{rows: BattleNetworkStatusRow[], text: string}|nil`
- `table|nil error`
