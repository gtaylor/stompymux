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
the internal function, including remote Wizard looks. Use `mux.contents`,
`mux.contents_visible`, `mux.exits`, `mux.exits_visible`, `mux.object_name`,
`mux.object_description`, and `mux.object_type` when assembling custom output.
There is no native opacity flag; use `external_appearance` to control what an
outside viewer sees.
