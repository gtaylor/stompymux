---
title: destination
type: docs
---

Returns an exit's destination.

## Method

```lua
local destination = exit:destination()
```

The receiver must be an exit. The method returns an [Object](../) handle for a
concrete destination, or `nil` when the exit is unlinked or its destination is
being destroyed. An invalid stored destination raises `mux.object.invalid`.

This method also raises `mux.object.invalid` when called on a room, thing, or
player, and `mux.unavailable.checking` during `@lua/check`.

## Example

```lua
local destination = exit:destination()
if destination then
  mux.log("lua", "Exit leads to " .. tostring(destination))
end
```
