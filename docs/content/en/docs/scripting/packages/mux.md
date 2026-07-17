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

Returns the string value of an attribute, or `nil` when the attribute is absent
or has an empty value.

```lua
local title = mux.attr_get(ctx.object, "Title") or "Untitled"
```

`object` must be a valid dbref and `name` must name an existing attribute.
Passing an invalid object raises a Lua error.

## `mux.attr_set(object, name, value)`

Sets an attribute to a string value. The attribute is created when necessary.

```lua
mux.attr_set(ctx.object, "LuaCount", "42")
```

`object` must be a valid dbref. Lua cannot use this function to set
`Luaparent`; use the wizard-only `@luaparent` command instead.

## `mux.notify(object, message)`

Sends a message to an object, normally the triggering player.

```lua
mux.notify(ctx.enactor, "The counter advances.")
```

`object` must be a valid dbref and `message` must be a string.

## `mux.command(command)`

Queues a MUX command to run after the current Lua callback completes.

```lua
mux.command("@emit A queued command from Lua.")
```

Queued commands execute as `#1`. They are asynchronous: the function does not
return a command result to Lua.

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

The `mux` table is the only server interface exposed to Lua modules. The Lua
sandbox does not expose filesystem, operating-system, debugger, FFI, coroutine,
or dynamic code-loading APIs. Handler instruction and state-memory limits still
apply while using these functions.
