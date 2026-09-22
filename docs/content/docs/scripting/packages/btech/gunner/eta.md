---
title: "btech.gunner.eta"
type: docs
linkTitle: "eta"
manualLinkTitle: "eta"
---

Estimate travel using parent speed and the station hex selection; notify station occupants.

## Signature

```lua
btech.gunner.eta(station, gunner, coordinates)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `station` | `integer` |  |
| `gunner` | `integer` |  |
| `coordinates` | `string?` | Same coordinate grammar as the corresponding unit report. |

## Returns

- `BattleEtaReport`
