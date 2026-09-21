---
title: "btech.unit.set_armor"
type: docs
---

Patch armor values on one section.

## Signature

```lua
btech.unit.set_armor(unit, section, patch)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `DbRef\|Object` |  |
| `section` | `BattleSection` | Typed section constant from btech.unit.sections. |
| `patch` | `table` | Current-armor, internal or rear-armor integers, each 0 through 255. |

## Returns

No values.
