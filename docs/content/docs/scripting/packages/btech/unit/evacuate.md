---
title: "btech.unit.evacuate"
type: docs
linkTitle: "evacuate"
manualLinkTitle: "evacuate"
---

Evacuate non-wizard contents of an in-character unit to the configured afterlife.
Requires a wizard actor and callback. Ordinary teleport hooks run; failures roll back moves and XP.
Tactical units do nothing. Configured XP retention applies only when in-character rules are enabled.

## Signature

```lua
btech.unit.evacuate(dbref, player)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Unit object. |
| `player` | `integer` | Wizard actor. |

## Returns

- `integer Number of occupants moved.`
