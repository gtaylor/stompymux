---
draft: true
title: set_display_name
type: docs
toc_hide: false
---

Sets a registered unit's display-name override. An empty string clears it.

```lua
btech.unit.set_display_name(unit, "Black Knight")
```

The caller must be a wizard. This function is deliberately unit-specific;
maps use their core object name and do not have a display-name override.
