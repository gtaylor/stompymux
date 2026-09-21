---
title: "btech.unit.section_condition"
type: docs
---

Read a section's damage condition.

## Signature

```lua
btech.unit.section_condition(unit, section)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `DbRef\|Object` |  |
| `section` | `BattleSection` | Typed section constant from btech.unit.sections. |

## Returns

- `"operational"|"destroyed"|"flooded" condition`
