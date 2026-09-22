---
title: "btech.template.armor"
type: docs
linkTitle: "armor"
manualLinkTitle: "armor"
---

Read current, original and rear armor values; an omitted section reports the totals.

## Signature

```lua
btech.template.armor(reference, section)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `reference` | `string` | Relative name under database.mech_database. |
| `section?` | `BattleSection` | Typed section constant from btech.unit.sections. |

## Returns

- `BattleArmorStatus status`
