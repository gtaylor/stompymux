---
title: "btech.unit.target"
type: docs
---

Select a section of the current unit target using its anatomical aliases.
Requires a running unit and its conscious assigned pilot. Nil or "-" clears without a lock.
The saved class and section persist across lock changes and shutdown. Returned values are detached.

## Signature

```lua
btech.unit.target(dbref, pilot, section)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `section` | `string\|nil` |  |

## Returns

- `BattleAimSelection|nil`
