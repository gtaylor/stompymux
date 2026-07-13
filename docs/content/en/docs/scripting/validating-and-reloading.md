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
`game.run/lua/object_logic`, `game.run/lua/global_commands`, and
`game.run/lua/packages`. It checks module syntax, top-level imports, and the
module return contract. Global command modules must export a `commands` table.

Checks run in a fresh Lua state and never replace the running state. Command
handlers and event handlers are not called. To prevent check-time side effects,
the `mux` API is unavailable while a module's top-level code is evaluated;
move any `mux` calls into a handler or event function before checking it.

## Reloading Lua at runtime

Use `@luareload` after a successful check to atomically put changed attached
and global command modules into service.
