---
draft: true
title: preferred_id
type: docs
---

Returns a registered unit's preferred two-letter tactical ID, or `nil` when
unset. The unit may be a dbref or `mux.world.Object`.

```lua
local id = btech.unit.preferred_id(unit)
```
