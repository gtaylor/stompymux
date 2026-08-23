---
title: remove
type: docs
---

```lua
local changed = object:powers():remove(mux.world.powers.IDLE)
```

Removes the supplied `Power`. Returns `true` when removed and `false` when it
was already absent.
