---
title: "btech.unit.lateral"
type: docs
linkTitle: "lateral"
manualLinkTitle: "lateral"
---

Request a six-second lateral change; requires an intact quad and its assigned pilot.

## Signature

```lua
btech.unit.lateral(dbref, player, direction)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `player` | `integer` |  |
| `direction` | `string` | nw/fl, ne/fr, sw/rl, se/rr, or - to travel straight. |

## Returns

- `Notice`
