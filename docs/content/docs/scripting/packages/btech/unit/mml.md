---
title: "btech.unit.mml"
type: docs
linkTitle: "mml"
manualLinkTitle: "mml"
---

Toggle an MML between SRM and LRM ammunition; requires dedicated matching bins.
SRMs use 3/6/9 range and two-point hits; LRMs use 7/14/21, minimum six, and five-point groups.

## Signature

```lua
btech.unit.mml(dbref, pilot, weapon)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `weapon` | `integer` | Zero-based weapon number. |

## Returns

- `BattleAmmunitionMode The selected supply: SRM rounds, or mml_lrm and its mml_lrm_* special rounds for LRM.`
