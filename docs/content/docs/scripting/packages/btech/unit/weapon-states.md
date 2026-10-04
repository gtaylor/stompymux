---
title: "btech.unit.weapon_states"
type: docs
linkTitle: "weapon_states"
manualLinkTitle: "weapon_states"
---

Inspect mounted weapons without acquiring targets or consuming dice; calling scripts own access policy.
Rust extension retained under its descriptive name; the canonical weapons list follows the C contract.

## Signature

```lua
btech.unit.weapon_states(dbref)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |

## Returns

- `WeaponInspection[] Lua array positions start at one; use each entry's index to fire.`
