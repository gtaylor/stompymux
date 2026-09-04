---
draft: true
title: ui_preferences
type: docs
---

Returns the effective display-preference table, including whether it is
explicitly configured.

```lua
local preferences = btech.player.ui_preferences(player)
if preferences.configured then
  -- The player has explicitly configured these values.
end
```

The table contains `tactical_height`, `tactical_width`, `lrs_height`, the five
`include_*` booleans, `buildings` (`follow_brief`, `include`, or `exclude`), and
the `configured` boolean.
