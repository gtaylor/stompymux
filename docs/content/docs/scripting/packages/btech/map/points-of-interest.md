---
title: "btech.map.points_of_interest"
type: docs
linkTitle: "points_of_interest"
manualLinkTitle: "points_of_interest"
---

List the map's scripted points of interest in file order. Units never see them.

## Signature

```lua
btech.map.points_of_interest(map, type)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `map` | `DbRef\|Object` |  |
| `type?` | `string` | Keep only points whose type matches exactly (case-sensitive). |

## Returns

- `BattleMapPointOfInterest[]`
