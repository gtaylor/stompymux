---
title: "btech.unit.set_team"
type: docs
linkTitle: "set_team"
manualLinkTitle: "set_team"
---

Wizard-only team edit for a placed unit; negatives normalize to zero, other signature facts are retained.

## Signature

```lua
btech.unit.set_team(actor, unit, team)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `unit` | `integer` |  |
| `team` | `integer` | Signed 32-bit team number. |

## Returns

- `integer team Normalized value, also reported privately to the administrator.`
