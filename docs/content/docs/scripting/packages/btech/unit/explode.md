---
title: "btech.unit.explode"
type: docs
linkTitle: "explode"
manualLinkTitle: "explode"
---

Start or stop cockpit self-destruction. Engagement releases the pilot assignment.
The same argument grammar, configuration and override checks apply as the native explode command.

## Signature

```lua
btech.unit.explode(dbref, pilot, argument)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `argument` | `string` | "ammo", "reactor", "stop", optionally followed by wizard "override". |

## Returns

- `boolean`
