---
draft: true
title: set_link
type: docs
---

Atomically stores a child map's parent placement and optional entrances. Pass
`nil` to clear it. `parent` accepts a dbref or `mux.world.Object`.

```lua
btech.map.set_link(child, {
  parent = parent,
  x = 10,
  y = 20,
  entrances = {
    north = { mode = "offset", offset = 3 },
    east = { mode = "exact", x = 0, y = 5 },
  },
})
```

Configuration changes do not rebuild live `BUILD`, `LEAVE`, or `ENTRANCE`
objects. Call `btech.map.update_links(parent)` after the desired edits.
