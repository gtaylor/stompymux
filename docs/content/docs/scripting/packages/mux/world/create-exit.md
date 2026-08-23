---
title: create_exit
type: docs
---

Creates an exit and attaches it to a source object.

## Function

```lua
mux.world.create_exit({
  name = "north;n",
  location = room,
  destination = north_room,
})
```

`name` and source `location` are required. The source must be capable of
holding exits. `destination` is optional; omitting it creates an attached but
unlinked exit. A destination must be capable of containing objects. References
may be dbrefs or [Object](../type-object/) handles.

The function returns the new `Object` with configured exit flags and default
Lua parent applied. It raises `mux.arg.invalid` for invalid options,
`mux.object.invalid` for invalid references or object kinds,
`mux.object.unavailable` for a source or destination being destroyed, and
`mux.unavailable.checking` during `@lua/check`.
