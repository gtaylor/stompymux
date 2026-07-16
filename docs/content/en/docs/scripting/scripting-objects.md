---
title: Object scripting
linkTitle: Objects
type: docs
weight: 10
---

Object Lua modules live under `game/lua/object_logic`. Attach one to an
object with the wizard-only command:

```text
@luaparent <object>=<path>.lua
```

The path is relative to `object_logic`; for example,
`@luaparent #123=hello.lua` selects
`game/lua/object_logic/hello.lua`. Paths must be relative `.lua` files and
cannot escape into `global_logic` or `packages`. Omit the path to clear an
attachment.

The closest `Luaparent` in an object's normal MUX parent chain supplies the
active module. Reload all attached modules and their dependencies atomically
with `@luareload`; a failed reload leaves the current Lua state running.
Use [`@luacheck`](validating-and-reloading/) to validate every Lua module
before reloading.

If an attached file is deleted or otherwise cannot load, startup logs the
object, configured path, and load error but continues. The attachment remains
in place; command matching and action events for that object log the load error
and are treated as handled, so legacy softcode does not run unexpectedly.
Restore the file or update `Luaparent`, then use `@luareload` to activate the
repair. `@luareload` itself remains atomic and rejects a missing attachment.

## Module contract

An object module returns a table with optional `commands` and `events` tables.
Command entries use native Lua patterns and a handler:

```lua
return {
  commands = {
    {
      pattern = "^hello$",
      handler = function(ctx)
        mux.notify(ctx.enactor, "Hello, world!")
        return true
      end,
    },
  },
  events = {},
}
```

Returning `true` handles the command; `false` or `nil` allows the local or zone
legacy `$` command search to continue. Object event functions are keyed by the
lowercase action-attribute name, such as `aenter`, `aleave`, `ahear`, and
`daily`. Attaching a module suppresses that object's matching legacy action
attribute, even if the module does not implement the event.

## Action and other-message events

The Lua event bridge is invoked for every action attribute supplied to
`did_it()`. The event key is the lowercased attribute name: for example,
`@asuccess` stores `Asucc`, which calls `events.asucc`.

The paired `@O` attributes are **other-message** attributes, not action
events. They continue to produce their existing evaluated messages while Lua
handles the paired `@A` action. They are included here as a migration
reference.

| Legacy action | Lua event | Trigger | Paired other message |
| --- | --- | --- | --- |
| `@asuccess` | `asucc` | A successful take, exit traversal, or lock-checked look. | `@osuccess` |
| `@afail` | `afail` | A failed take, exit traversal, or lock-checked look. | `@ofail` |
| `@adrop` | `adrop` | An object is dropped. | `@odrop` |
| `@agfail` | `agfail` | Giving an object fails its give lock. | `@ogfail` |
| `@arfail` | `arfail` | Giving to a recipient fails its receive lock. | `@orfail` |
| `@adfail` | `adfail` | Dropping an object fails. | `@odfail` |
| `@ause` | `ause` | An object is used. | `@ouse` |
| `@aufail` | `aufail` | Using an object fails its use lock. | `@oufail` |
| `@adescribe` | `adesc` | A description or inside description is displayed. | `@odescribe` |
| `@aenter` | `aenter` | An object or room is entered. | `@oenter` |
| `@aleave` | `aleave` | An object or room is left. | `@oleave` |
| `@amove` | `amove` | A player or thing completes a move. | `@omove` |
| `@aefail` | `aefail` | Entering an object fails its enter lock. | `@oefail` |
| `@alfail` | `alfail` | Leaving an object fails its leave lock. | `@olfail` |
| `@atport` | `atport` | A player or thing teleports successfully. | `@otport` |
| `@atfail` | `atfail` | Teleporting to a destination fails. | `@otfail` |
| `@atofail` | `atofail` | Teleporting out of an origin fails. | `@otofail` |
| `@aahear` | `aahear` | A matching `@listen` message is heard, including the speaker. | — |
| `@ahear` | `ahear` | A matching `@listen` message is heard from someone else. | — |
| `@amhear` | `amhear` | A matching `@listen` message is spoken by the object itself. | — |
| `@aclone` | `aclone` | An object is cloned. | — |
| `@startup` | `startup` | The server starts. | — |
| `@daily` | `daily` | The daily timer runs. | — |
| `HHourly` | `hhourly` | The hourly timer runs. | — |
| `@amechdest` | `amechdest` | A BattleTech mech is destroyed. | — |
| `@aminetrigger` | `aminetrigger` | A BattleTech mine is triggered. | — |
| `@aaeroland` | `aaeroland` | A BattleTech aerospace unit lands. | — |
| `@aoodland` | `aoodland` | A BattleTech out-of-danger landing completes. | — |

`@oxenter`, `@oxleave`, and `@oxtport` are the other-message notifications
for the location being left; they have no paired action attribute. All other
`@O` attributes are listed in the table above.

`@aconnect`, `@adisconnect`, and `@runout` remain legacy action attributes,
but do not currently pass through `did_it()` and therefore do not yet invoke a
Lua event. Connection and charge-exhaustion hooks should be migrated
separately.

See [Commands](commands/) for Lua-pattern syntax, capture arguments, and the
handler context table.

## Imports and examples

Object modules resolve `require("name")` in `object_logic` before looking in
the shared `packages` root. See `game/lua/object_logic/` for the `hello`,
`counter`, and `events/enter_notice` examples.

The [`mux` package](packages/mux/) documents the server API available to
object modules.

## Scheduled events

Object modules may define a `schedules` array. Each entry has a unique `name`,
a numeric five-field UTC cron expression, and a handler. Cron fields accept
`*`, values, lists, ranges, and steps. Matching jobs are spread deterministically
across the first 55 seconds of the minute and are not replayed after downtime.

```lua
schedules = {
  {
    name = "hourly_notice",
    cron = "0 * * * *",
    handler = function(ctx)
      -- ctx.scope == "object" and ctx.object is the effective object.
    end,
  },
}
```

A shared `Luaparent` runs each matching schedule once for every object that
inherits it. Use the wizard-only `@luaschedule` command to inspect active
schedules and their effective objects.
