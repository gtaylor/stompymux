---
title: "btech.unit.heading"
type: docs
linkTitle: "heading"
manualLinkTitle: "heading"
---

Set desired heading on the current pilot's running unit and stage a cockpit confirmation.

## Signature

```lua
btech.unit.heading(dbref, player, degrees)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `player` | `integer` |  |
| `degrees` | `number?` | Omit to read actual heading without notification. |

## Returns

- `boolean|number`
