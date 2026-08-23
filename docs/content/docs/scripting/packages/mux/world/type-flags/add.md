---
title: add
type: docs
---

```lua
local changed = object:flags():add(mux.world.flags.SAFE)
```

Sets the supplied `Flag`. Returns `true` when it was newly set and `false` when
it was already present. Native-policy rejection raises `mux.object.unavailable`.
