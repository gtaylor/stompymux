---
title: "btech.unit.weapon_diagnostics"
type: docs
linkTitle: "weapon_diagnostics"
manualLinkTitle: "weapon_diagnostics"
---

Inspect durable equipment condition; empty ammunition, shutdown and recycle do not imply damage.
Requires a trusted callback transaction. Returned rows are detached from saved unit state.

## Signature

```lua
btech.unit.weapon_diagnostics(dbref)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |

## Returns

- `BattleWeaponDiagnostic[]`
