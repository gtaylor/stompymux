---
title: "btech.map.terrain"
type: docs
linkTitle: "terrain"
manualLinkTitle: "terrain"
---

Read the one terrain feature a map shows for a hex: fire or smoke, then a structure, water, woods or the ground. Use btech.map.hex for every layer, including the terrain beneath fire or smoke.

## Signature

```lua
btech.map.terrain(map, hex)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `map` | `DbRef\|Object` |  |
| `hex` | `BattleHexCoordinate` |  |

## Returns

- `BattleTerrainName terrain`
