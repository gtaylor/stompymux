---
title: create_thing
type: docs
---

Creates a thing, establishes its home, and places it in a container.

## Function

```lua
mux.world.create_thing({
  name = "Console",
  location = room,
  home = room,
})
```

`name` and `location` are required. `location` accepts a dbref or
[Object](../type-object/) capable of containing objects. `home` accepts the
same types and defaults to `location`. All fields are validated before the
object is allocated.

The function returns the new `Object` with configured thing flags and default
Lua parent applied. It raises `mux.arg.invalid` for invalid options,
`mux.object.invalid` for invalid references or object kinds,
`mux.object.unavailable` for a container being destroyed, and
`mux.unavailable.checking` during `@lua/check`.
