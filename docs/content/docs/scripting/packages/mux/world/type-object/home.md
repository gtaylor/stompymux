---
title: home
type: docs
---

Returns a thing or player's assigned home.

## Method

```lua
local home = object:home()
```

The receiver must be a thing or player. The method returns an [Object](../)
handle for its home, or `nil` when no home is assigned or the home is being
destroyed.

It raises `mux.object.invalid` for other receiver types or an invalid stored
home, and `mux.unavailable.checking` during `@lua/check`.

## Example

```lua
local home = object:home()
if home then
  mux.log("lua", tostring(object) .. " is homed to " .. tostring(home))
end
```
