---
title: "btech.unit.section_condition"
type: docs
linkTitle: "section_condition"
manualLinkTitle: "section_condition"
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
| `section` | `MechSection` | Typed section constant from btech.unit.sections. |

## Returns

- `"operational"|"destroyed"|"flooded" condition`
