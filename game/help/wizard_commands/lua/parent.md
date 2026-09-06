+++
title = "@lua/parent"
description = "Attach or clear an object's Lua parent"
keywords = ["@lua/parent"]
article_tags = ["lua_switches"]
weight = 20
wizard_only = true
+++

# @lua/parent

Attach an object-logic module to an object, or omit the path to clear the
attachment:

```text
@lua/parent <object>=<path>.lua
@lua/parent <object>=
```

Paths are relative to `game/lua/object_logic`. The attachment applies only to
that object; object-logic modules are not inherited through other objects.

New objects receive the type-specific Lua parent configured by
`default_thing_lua_parent`, `default_room_lua_parent`,
`default_exit_lua_parent`, or `default_player_lua_parent`. Changing these
settings affects only objects created afterward. Existing objects are not
updated, and `@clone` preserves the source object's Lua parent instead of using
the configured default.

An object module may export `internal_appearance(ctx)` and
`external_appearance(ctx)`. Returning a string replaces the complete native
`look` appearance; returning `nil` uses the native appearance. Rooms always use
the internal function. Use `mux.world.object(ctx.object)`, `Object:contents`
filters, and typed `mux.world.types` constants when assembling custom output.
`Object:contents()` includes both ordinary contents and attached exits; pass a
`types` filter when only one group should be shown.
Use `mux.world.lock_passes` with a typed
`mux.world.locks` constant when output depends on a lock result.
There is no native opacity flag; use `external_appearance` to control what an
outside viewer sees.

The bundled default room parent lists visible players other than the viewer in
a left column beside visible exits, then lists non-player contents below. If no
other players are visible, the exits column remains right-aligned.
