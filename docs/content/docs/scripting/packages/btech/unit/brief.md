---
title: "btech.unit.brief"
type: docs
---

Query unit display settings or edit A/C independently. Edits notify occupants.
Requires conscious cockpit occupant; shutdown is allowed. Errors roll back state and notices.

## Signature

```lua
btech.unit.brief(unit, pilot, arguments)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `integer` |  |
| `pilot` | `integer` |  |
| `arguments` | `string?` | Empty for query, A 0..6 or C 0..3 for edits. |

## Returns

- `BattleBriefReport`
