---
title: "btech.unit.snipe"
type: docs
linkTitle: "snipe"
manualLinkTitle: "snipe"
---

Wizard-only predictive firing using fixed horizontal target orders and normal weapon launches.
Sets the cockpit hex target. Does not simulate future damage or order changes.

## Signature

```lua
btech.unit.snipe(dbref, player, target, selection)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` | Shooter unit |
| `player` | `integer` | Assigned wizard pilot |
| `target` | `integer` | Target unit on the same battlefield |
| `selection` | `string` | Comma-separated weapon numbers and inclusive ranges |

## Returns

- `boolean success`
