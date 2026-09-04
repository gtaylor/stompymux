---
title: ui_preferences
type: docs
---

Returns the effective display-preference table and a boolean indicating whether
it is explicitly configured.

```lua
local preferences, configured = btech.player.ui_preferences(player)
```

The table contains `tactical_height`, `tactical_width`, `lrs_height`, the five
`include_*` booleans, and `buildings` (`follow_brief`, `include`, or `exclude`).
