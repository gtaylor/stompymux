---
title: "btech.player.view_dimensions"
type: docs
linkTitle: "view_dimensions"
manualLinkTitle: "view_dimensions"
---

Read saved map dimensions or replace them; omitted fields in a replacement use standard defaults.
Trusted callback code owns authorization to change the selected player's preferences.
Native tactical/LRS and Lua tactical/LRS/viewport use these defaults; navigate stays radius two.

## Signature

```lua
btech.player.view_dimensions(player, dimensions)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `player` | `integer` | Live player dbref. |
| `dimensions` | `BattleViewDimensions?` | Validated replacement; omit for a read-only query. |

## Returns

- `BattleViewDimensions`
