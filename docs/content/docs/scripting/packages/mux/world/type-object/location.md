---
title: location
type: docs
---

Returns a thing or player's current location.

## Method

```lua
local location = object:location()
```

The receiver must be a thing or player. The method returns an [Object](../)
handle for its location, or `nil` when it has no location or the location is
being destroyed. Use [`Object:destination`](../destination/) for an exit; room
droptos are not exposed by this method.

It raises `mux.object.invalid` for other receiver types or an invalid stored
location, and `mux.unavailable.checking` during `@lua/check`.

## Example

```lua
local location = object:location()
if location then
  mux.log("lua", tostring(object) .. " is in " .. tostring(location))
end
```
