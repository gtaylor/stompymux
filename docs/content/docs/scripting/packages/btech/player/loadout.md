---
title: loadout
type: docs
---

Returns the player's personal-combat loadout, or `nil` when none is configured.
The result has an `armor` table and optional `right` and `left` equipment.

```lua
local loadout = btech.player.loadout(player)
```
