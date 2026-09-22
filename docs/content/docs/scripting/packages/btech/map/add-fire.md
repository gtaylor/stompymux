---
title: "btech.map.add_fire"
type: docs
linkTitle: "add_fire"
manualLinkTitle: "add_fire"
---

Install wizard fire; zero duration is permanent. Off-map coordinates leave the map unchanged.

## Signature

```lua
btech.map.add_fire(actor, dbref, x, y, duration)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `actor` | `integer` |  |
| `dbref` | `integer` |  |
| `x` | `integer` |  |
| `y` | `integer` |  |
| `duration` | `integer` | Signed seconds; fire keeps a signed-short spread budget, smoke uses at least one tick. |

## Returns

- `boolean`
