---
title: has
type: docs
---

```lua
local present = object:powers():has(mux.world.powers.IDLE)
```

Returns whether the supplied `Power` is granted. Strings, flags, and other
values raise `mux.power.invalid`.
