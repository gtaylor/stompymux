---
draft: true
title: set_value
type: docs
toc_hide: false
---

Writes a script-visible field on a registered live unit.

```lua
btech.unit.set_value(unit, name, value)
```

The caller must be a wizard. Field names are matched without regard to ASCII
case; unsupported fields and non-unit targets raise a Lua error.
