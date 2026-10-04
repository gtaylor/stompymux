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
| `reference` | `string` | Template reference: the file stem of a `.toml` document anywhere under database.mech_database. |
| `section?` | `UnitSection` | Typed section constant from btech.unit.sections. |

## Returns

- `ArmorStatus status`
