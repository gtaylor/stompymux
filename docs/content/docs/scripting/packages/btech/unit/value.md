---
draft: true
title: value
type: docs
toc_hide: false
---

Reads a script-visible field from a registered live unit.

```lua
local id = btech.unit.value(unit, "id")
```

Field names are matched without regard to ASCII case. Unsupported fields and
non-unit targets raise a Lua error.
