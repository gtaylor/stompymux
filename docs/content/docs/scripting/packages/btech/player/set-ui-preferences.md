---
draft: true
title: set_ui_preferences
type: docs
---

Atomically stores a complete display-preference table. Pass `nil` to restore
defaults. Heights must be 5–24 for tactical, 5–40 for tactical width, and
10–40 for LRS.

```lua
btech.player.set_ui_preferences(player, {
  tactical_height = 14, tactical_width = 21, lrs_height = 11,
  include_dead = false, include_shutdown = true,
  include_enemies = true, include_allies = true, include_target = true,
  buildings = "exclude",
})
```
