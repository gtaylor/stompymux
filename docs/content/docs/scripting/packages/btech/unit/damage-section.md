---
title: "btech.unit.damage_section"
type: docs
linkTitle: "damage_section"
manualLinkTitle: "damage_section"
---

Wizard-only located damage through shared critical, crew and evacuation rules.

## Signature

```lua
btech.unit.damage_section(actor, unit, section, damage, rear, critical)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `unit` | `integer` |  |
| `section` | `string` | Chassis-specific location or abbreviation. |
| `damage` | `integer` | 1 through 1000. |
| `rear` | `boolean` | Rear armor selection; vehicle front hits redirect to rear. |
| `critical` | `boolean` | Through-armor critical candidate. |

## Returns

- `{kind: "mech"|"vehicle", impact: table} report`
