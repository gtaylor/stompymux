---
title: "btech.map.add_smoke"
type: docs
linkTitle: "add_smoke"
manualLinkTitle: "add_smoke"
---

Install wizard smoke; zero duration is permanent. Off-map coordinates leave the map unchanged.

## Signature

```lua
btech.map.add_smoke(actor, dbref, x, y, duration)
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
