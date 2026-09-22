---
title: "btech.unit.damage"
type: docs
linkTitle: "damage"
manualLinkTitle: "damage"
---

Wizard-only random damage packets through shared combat and casualty rules.

## Signature

```lua
btech.unit.damage(actor, unit, damage, clusters, rear, critical)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `unit` | `integer` | Power, pilot and placement are not required. |
| `damage` | `integer` | 1 through 1000. |
| `clusters` | `integer` | Packet count from 1 through damage; integer division discards the remainder. |
| `rear` | `boolean` | Select rear armor, also enabled by a rear incoming arc. |
| `critical` | `boolean` | Accepted flag; random location routing chooses critical eligibility. |

## Returns

- `{packet_damage: integer, discarded_damage: integer, impacts: table[]} report`
