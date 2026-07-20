---
title: Object scripting
linkTitle: Objects
type: docs
weight: 10
---

Object Lua modules live under `game/lua/object_logic`. Attach one to an
object with the wizard-only command:

```text
@lua/parent <object>=<path>.lua
```

The path is relative to `object_logic`; for example,
`@lua/parent #123=hello.lua` selects
`game/lua/object_logic/hello.lua`. Paths must be relative `.lua` files and
cannot escape into `global_logic` or `packages`. Omit the path to clear an
attachment.

The closest `Luaparent` in an object's normal MUX parent chain supplies the
active module. Reload all attached modules and their dependencies atomically
with `@lua/reload`; a failed reload leaves the current Lua state running.
Use [`@lua/check`](validating-and-reloading/) to validate every Lua module
before reloading.

If an attached file is deleted or otherwise cannot load, startup logs the
object, configured path, and load error but continues. The attachment remains
in place; command matching and native events for that object log the load error
and are treated as handled.
Restore the file or update `Luaparent`, then use `@lua/reload` to activate the
repair. `@lua/reload` itself remains atomic and rejects a missing attachment.

When a Wizard uses `@examine` on an object, the output identifies its effective
Lua parent and attachment object, then lists its command patterns, events,
schedule names, message providers, and locks. Use `@lua/viewparent <dbref>` to
display that module's raw source, or `@lua/viewparent <path>.lua` to inspect an
object-logic module directly by path.

## Module contract

An object module returns a table with optional `commands`, `events`, `locks`,
`messages`, and `schedules` tables.
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
legacy `$` command search to continue. Object event functions use the native
Lua event names listed below. Event names are validated by `@lua/check`; an
unknown name or non-function value is an error.

## Object locks

Define locks as functions in the module's `locks` table. The supported keys
are `default`, `drop`, `enter`, `give`, `leave`, `link`, `receive`, `speech`,
`teleport`, `teleport_out`, and `use`.

```lua
locks = {
  enter = function(ctx)
    if ctx.subject == 1 then
      return true
    end
    return {
      passes = false,
      enactor_message = "You cannot enter here.",
      other_message = "tries to enter, but cannot.",
    }
  end,
}
```

A lock may return a boolean or a table. A table must contain boolean `passes`
and may contain string `enactor_message` and `other_message`. Omitted messages
use the native default for the attempted action; an empty string suppresses
that message. On failure, messages are delivered first and the corresponding
`on_*_fail` event then runs on the object whose lock failed.

The context includes the normal `object`, `enactor`, `cause`, and `descriptor`
fields, plus `subject` (the object being tested), `lock`, `operation`, and
`silent`. `operation` distinguishes uses of the same semantic lock. Its values
are `match`, `traverse`, `take`, `look`, `command_match`, `listen`, `use`,
`drop`, `give`, `receive`, `enter`, `leave`, `teleport`, `teleport_out`,
`link`, `set_home`, `speak`, `zone_control`, `channel_join`,
`channel_transmit`, `channel_receive`, `btech_enter`, and `btech_contact`.
Lock handlers have access to the full [`mux`](packages/mux/) API.

An absent lock handler passes. An attached module that cannot load, a runtime
error, or a malformed lock result fails closed and is logged. Native
`Pass_Locks` and other built-in authorization bypasses still apply.

Lock result messages are used only when a lock fails. Successful actions use
the message providers described below.

## Action messages

Define successful-action messages as functions in the module's `messages`
table. The supported keys are `success`, `drop`, `describe`, `use`, `leave`,
`enter`, `move`, `teleport`, `enter_source`, `leave_destination`, and
`teleport_source`.

```lua
messages = {
  use = function(ctx)
    return {
      enactor_message = "You activate the console.",
      other_message = "activates the console.",
    }
  end,
}
```

A provider returns a table with optional string `enactor_message` and
`other_message` fields. An omitted provider or field uses the native default;
an empty string suppresses that message. `describe`, `enter_source`,
`leave_destination`, and `teleport_source` accept only `other_message` because
they replace legacy messages that never notified the enactor. A load error,
runtime error, or malformed result is logged and also uses the native default;
the action continues.

The context includes the normal object fields, plus `message`, `operation`,
`silent`, `source`, and `destination`. `source` or `destination` is `nil` when
it does not apply. Operation values are `none`, `look`, `take`, `traverse`,
`receive`, `drop`, `give`, `describe`, `inside_describe`, `use`, `move`, and
`teleport`.

The cross-location providers preserve the old movement notification scopes:
`enter_source` belongs to the destination and speaks in the source before a
move, `leave_destination` belongs to the source and speaks in the destination
after a move, and `teleport_source` belongs to the moving object and speaks in
its source before a teleport. The native "has left" and "has arrived"
announcements are separate and remain unchanged.

## Object events

Native game behavior invokes Lua events directly. Action message providers run
and deliver their messages before the corresponding Lua handler. Lock failure
messages come from the structured lock result or the native defaults.

| Lua event | Trigger | Message before event |
| --- | --- | --- |
| `on_success` | A successful take, exit traversal, or lock-checked look. | `messages.success` |
| `on_fail` | A failed take, exit traversal, or lock-checked look. | Lock result |
| `on_drop` | An object is dropped. | `messages.drop` |
| `on_give_fail` | Giving an object fails its give lock. | Lock result |
| `on_give_receive_fail` | Giving to a recipient fails its receive lock. | Lock result |
| `on_drop_fail` | Dropping an object fails. | Lock result |
| `on_use` | An object is used. | `messages.use` |
| `on_use_fail` | Using an object fails its use lock. | Lock result |
| `on_describe` | A description or inside description is displayed. | `messages.describe` |
| `on_enter` | An object or room is entered. | `messages.enter` |
| `on_leave` | An object or room is left. | `messages.leave` |
| `on_move` | A player or thing completes a move. | `messages.move` |
| `on_enter_fail` | Entering an object fails its enter lock. | Lock result |
| `on_leave_fail` | Leaving an object fails its leave lock. | Lock result |
| `on_teleport` | A player or thing teleports successfully. | `messages.teleport` |
| `on_teleport_destination_fail` | Teleporting to a destination fails. | Lock result |
| `on_teleport_out_fail` | Teleporting out of an origin fails. | Lock result |
| `on_match_heard` | A matching `@listen` message is heard, including the speaker. | — |
| `on_match_heard_other` | A matching `@listen` message is heard from someone else. | — |
| `on_match_heard_self` | A matching `@listen` message is spoken by the object itself. | — |
| `on_clone` | An object is cloned. | — |
| `on_server_startup` | The server starts. | — |
| `on_connect` | A player connects or reconnects. | — |
| `on_disconnect` | A player's final descriptor disconnects. | — |
| `on_mech_destroyed` | A BattleTech mech is destroyed. | — |
| `on_mech_mine_trigger` | A BattleTech mine is triggered. | — |
| `on_aero_land` | A BattleTech aerospace unit lands. | — |
| `on_ood_land` | A BattleTech out-of-danger landing completes. | — |

Movement also invokes the applicable cross-location message providers, which
do not have corresponding events.

Connection events run for the player's effective module, the master room and
its contents, and the applicable zone object or zone-room contents. Both
receive `ctx.descriptor`. `on_connect` also receives boolean `ctx.reconnect`;
`on_disconnect` receives string `ctx.reason` and runs only for the final active
descriptor.

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
inherits it. Use the wizard-only `@lua/schedule` command to inspect active
schedules and their effective objects.
