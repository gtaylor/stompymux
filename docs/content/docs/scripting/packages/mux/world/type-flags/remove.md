---
title: remove
type: docs
---

```lua
local changed = object:flags():remove(mux.world.flags.SAFE)
```

Clears the supplied `Flag`. Returns `true` when it was removed and `false` when
it was already absent. Native-policy rejection raises
`mux.object.unavailable`.
