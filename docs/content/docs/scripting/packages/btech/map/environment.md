---
title: "btech.map.environment"
type: docs
---

Wizard SETCOND action; updates live map rules without advancing time or resetting units.

## Signature

```lua
btech.map.environment(actor, dbref, conditions)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `dbref` | `integer` |  |
| `conditions` | `BattleMapEnvironment` |  |

## Returns

- `BattleMapEnvironment Actual resulting state, including retained underground status.`
