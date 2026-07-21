---
title: mux package
linkTitle: mux
type: docs
weight: 10
---

# `mux`

`mux` is the built-in table available to every Lua module. It is supplied by
the game server rather than loaded with `require`. Lua callbacks and commands
run with the server's `#1` authority model; scripts should still use the
callback context to identify the object and enactor that triggered them.

## `mux.attr_get(object, name)`

Returns the string value of a dynamic storage entry, or `nil` when it is
absent. Names are exact and case-sensitive, so `Title` and `title` are distinct.

```lua
local title = mux.attr_get(ctx.object, "Title") or "Untitled"
```

`object` must be a valid dbref. Passing an invalid object raises a Lua error.

## `mux.attr_set(object, name, value)`

Sets a dynamic storage entry to a string value. The entry is created when
necessary; an empty value deletes it. Names must begin with a letter, may use
the server's printable attribute-name characters, and are limited to 255 bytes.

```lua
mux.attr_set(ctx.object, "LuaCount", "42")
```

`object` must be a valid dbref. Dynamic entries have no flags, owners,
inheritance, reserved names, or native server behavior.

## `mux.contents(object)`

Returns an array of dbrefs directly contained by a room, thing, or player in
native database order. The result is deliberately unfiltered.

```lua
for _, member in ipairs(mux.contents(ctx.object)) do
  if mux.contents_visible(ctx.object, ctx.enactor, member) then
    mux.notify(ctx.enactor, mux.object_name(member))
  end
end
```

## `mux.contents_visible(container, viewer, member)`

Returns whether native `look` would display `member` in `container` to
`viewer`, including location darkness, object darkness, disconnected-player,
self, and examinability rules. `member` must be directly contained by
`container`.

## `mux.exits(object)`

Returns an unfiltered array of exits directly attached to a room, thing, or
player in native database order. Legacy MUX parent exits are not inherited.

```lua
for _, exit in ipairs(mux.exits(ctx.object)) do
  if mux.exits_visible(ctx.object, ctx.enactor, exit) then
    mux.notify(ctx.enactor, mux.object_name(exit))
  end
end
```

## `mux.exits_visible(location, viewer, exit)`

Returns whether native `look` would display a directly attached exit to the
viewer. The exit must belong directly to `location`.

## `mux.object_name(object)`

Returns the object's stored name. Exit names include their semicolon-separated
aliases.

## `mux.object_description(object)`

Returns the object's native MUX `description` value, or `nil` when it is not
set. This is separate from the exact-name dynamic storage read by
`mux.attr_get`.

## `mux.object_type(object)`

Returns `room`, `thing`, `exit`, or `player`.

All object arguments must be valid, non-garbage dbrefs. Passing a container of
the wrong type or a member that is not directly attached raises a Lua error.

## `mux.notify(object, message)`

Sends a message to an object, normally the triggering player.

```lua
mux.notify(ctx.enactor, "The counter advances.")
```

`object` must be a valid dbref and `message` must be a string.

## `mux.connected_players()`

Returns an array of player connections visible to the normal `who` command.
Each entry has `name`, `connected_for`, and `idle_for` fields. The duration
fields are elapsed seconds.

```lua
for _, player in ipairs(mux.connected_players()) do
  mux.notify(ctx.enactor, player.name)
end
```

The result does not expose hidden players or privileged connection details.

## `mux.who_summary()`

Returns the non-privileged WHO summary table with `hidden`, `record`, and
`maximum` fields. `maximum` is `nil` when the game has no player limit.

```lua
local summary = mux.who_summary()
local maximum = summary.maximum or "no"
```

## `mux.flow_start(descriptor, module, first_step)`

Attaches an [interactive flow](../flows/) to a descriptor and shows its
first prompt.

```lua
mux.flow_start(ctx.descriptor, "confirm_delete.lua", "confirm")
```

`descriptor` is normally `ctx.descriptor` from the calling command or event.
Raises a Lua error if the descriptor doesn't exist, already has a flow
running, or `module` has no `first_step` in its `flows` table.

## Availability and limits

The `mux` table is the only server interface exposed to Lua modules. Runtime
database functions, including object enumeration and identity, are unavailable
during `@lua/check`. The Lua
sandbox does not expose filesystem, operating-system, debugger, FFI, coroutine,
or dynamic code-loading APIs. Handler instruction and state-memory limits still
apply while using these functions.
