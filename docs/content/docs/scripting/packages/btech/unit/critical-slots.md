---
title: "btech.unit.critical_slots"
type: docs
linkTitle: "critical_slots"
manualLinkTitle: "critical_slots"
---

List one section's critical slots with resolved parts, modes and ammunition state.

## Signature

```lua
btech.unit.critical_slots(unit, section)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `DbRef\|Object` |  |
| `section` | `BattleSection` | Typed section constant from btech.unit.sections. |

## Returns

- `BattleCriticalSlot[] slots`
