---
title: set_lua_parent
---

Assigns an object's direct Lua parent or clears the assignment.

```lua
object:set_lua_parent("events/enter_notice.lua")
object:set_lua_parent(nil)
```

## Parameters

`string|nil parent`
: An existing `.lua` file relative to `game/lua/object_logic`, or `nil` to
  clear the current assignment. Do not include an `object_logic/`,
  `global_logic/`, or `packages/` prefix.

The receiver is the live database object to update. The parent argument is
required even when clearing an assignment. The path is resolved with the same
validation as `@lua/parent`: it must remain inside the object-logic root and
the file must exist. This trusted world mutation does not perform the command's
player-control checks.

## Returns

No values.

## Errors and availability

Raises `mux.arg.invalid` for an omitted parent, an unsupported value type, or
an embedded NUL byte; `mux.module.invalid` for an invalid or unavailable path;
`mux.object.invalid` for an invalid object reference; and
`mux.object.unavailable` when the receiver is being destroyed or the value
cannot be stored. This method is unavailable during `@lua/check` and raises
`mux.unavailable.checking` there.
