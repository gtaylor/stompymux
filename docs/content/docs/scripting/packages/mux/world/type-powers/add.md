---
title: add
type: docs
---

```lua
local changed = object:powers():add(mux.world.powers.IDLE)
```

Grants the supplied `Power`. Returns `true` when newly granted and `false` when
it was already present.
