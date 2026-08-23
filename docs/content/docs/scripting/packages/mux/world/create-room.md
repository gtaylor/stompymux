---
title: create_room
type: docs
---

Creates a detached room using the configured room flags and default Lua parent.
Rooms do not have containment locations, so `location` is not accepted.

## Function

```lua
mux.world.create_room({ name = "Operations" })
```

`name` is required and may contain valid styled-text markup. The function
returns the new [Object](../type-object/). Unknown fields and invalid names
raise `mux.arg.invalid`.

This trusted operation is available only at runtime and raises
`mux.unavailable.checking` during `@lua/check`.

## Example

```lua
local room = mux.world.create_room({ name = "[bold]Operations[/]" })
assert(room.type == "room")
```
