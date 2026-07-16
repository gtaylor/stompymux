---
title: Validating and Reloading
linkTitle: Validating and Reloading
type: docs
weight: 30
---

You can validate and reload your game's Lua files from within the game without a full restart.

## Validating Lua

To check your Lua scripts for validity, run the following from in-game with a Wizard character:

```text
@luacheck
```

`@luacheck` recursively verifies every `.lua` file below
`game/lua/object_logic`, `game/lua/global_logic`, and
`game/lua/packages`. It checks module syntax, top-level imports, and the
module return contract, including cron schedules. Global logic modules must
export a nonempty `commands` or `schedules` table. It also checks every
configured `Luaparent` path. Missing or unreadable paths
are reported once per path with the number of objects that use that value, so a
single deleted file does not produce one error for every affected object.

Checks run in a fresh Lua state and never replace the running state. Command
handlers and event handlers are not called. To prevent check-time side effects,
the `mux` API is unavailable while a module's top-level code is evaluated;
move any `mux` calls into a handler or event function before checking it.

## Reloading Lua at runtime

Use `@luareload` after a successful check to atomically put changed attached
and global logic modules into service.

## Inspecting schedules

Wizards can use `@luaschedule` to list scheduled object modules with their
effective-object counts and scheduled global logic modules. Pass an object to
show its effective Lua parent, a relative `object_logic` path to show its
schedules and inheriting objects, or `global_logic/<path>.lua` to inspect one
global module.
