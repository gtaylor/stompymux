---
draft: true
title: value_ref
type: docs
toc_hide: false
---

Reads a script-visible field from a unit template.

```lua
local id = btech.unit.value_ref("mechs/example", "id")
```

The caller must be a wizard. Field names are matched without regard to ASCII
case; unsupported fields and missing templates raise a Lua error.
