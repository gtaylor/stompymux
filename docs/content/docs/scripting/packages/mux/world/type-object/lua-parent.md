---
title: lua_parent
---

Returns an object's directly assigned Lua parent path.

```lua
local parent = object:lua_parent()
```

The receiver is the live database object to inspect.

## Returns

`string|nil parent`
: The path relative to `game/lua/object_logic`, or `nil` when the object has no
  Lua parent. The returned path does not include the `object_logic/` prefix.

This reports only the object's direct assignment. Object-logic modules are not
inherited through zones, locations, or other objects.

## Errors and availability

Raises `mux.object.invalid` for an invalid receiver. This method is
unavailable during `@lua/check` and raises `mux.unavailable.checking` there.
