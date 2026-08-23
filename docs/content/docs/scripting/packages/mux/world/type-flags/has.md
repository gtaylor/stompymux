---
title: has
type: docs
---

```lua
local present = object:flags():has(mux.world.flags.SAFE)
```

Returns whether the supplied `Flag` is set. Strings, powers, and other values
raise `mux.flag.invalid`.
