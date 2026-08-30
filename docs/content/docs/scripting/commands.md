---
title: Commands
linkTitle: Commands
description: How to define new global and object-level commands
type: docs
weight: 15
---

Lua command modules return a `commands` array. Each entry has a native Lua
`pattern`, a `handler` function, and an optional `access` level:

```lua
return {
  commands = {
    {
      pattern = "^say%s+(.+)$",
      access = mux.world.access.WIZARD,
      handler = function(ctx, message)
        mux.world.pemit(ctx.enactor, "You said: " .. message)
        return true
      end,
    },
  },
}
```

Omit `access`, or set it to [`mux.world.access.PUBLIC`](packages/mux/world/access/),
to allow everyone. Use `mux.world.access.WIZARD` to allow Wizards and God, or
`mux.world.access.GOD` to allow only God. Raw strings are rejected. Invalid
values cause module validation and reload to fail.

An entry the invoker cannot access is skipped silently before its pattern or
handler runs. Matching continues with later entries and command scopes.

Programmable commands must be defined in Lua. Persistent object state is data
only and is never matched as a command.

## Pattern matching

Patterns use Lua's `string.match` syntax, not MUX wildcards or regular
expressions. Matching is case-sensitive. Anchor a pattern with `^` and `$`
when it must match the complete command.

Common Lua pattern elements include:

| Pattern | Meaning |
| --- | --- |
| `.` | Any character |
| `%s` | Whitespace |
| `%d` | Digit |
| `+` | One or more repetitions |
| `*` | Zero or more repetitions |
| `(...)` | Captures a value for the handler |

The pattern is matched against the otherwise-unmatched command text. Each
capture becomes an argument after `ctx`, in capture order. When a pattern has
no explicit captures, Lua passes the complete match as the argument after
`ctx`.

```lua
{
  pattern = "^roll%s*(%d*)$",
  handler = function(ctx, sides)
    sides = tonumber(sides) or 6
    -- ...
    return true
  end,
}
```

## Handling results and order

A handler returns `true` to handle the command. Returning `false` or `nil`
leaves it unhandled.

For object modules, all matching entries run in declaration order. If no object
handler handles the command, matching continues through the remaining local and
zone Lua scopes. A Lua pattern or handler error is logged and counts as handled.

Global command modules run only after local and zone Lua matching declines the
command. Their modules are checked in lexical path order and stop at the first
handler that returns `true`.

## Command context

Every command handler receives a context table as its first argument.

| Field | Local object command | Global command | Description |
| --- | --- | --- | --- |
| `ctx.object` | dbref | `nil` | The command-scope object whose active module handled the command. |
| `ctx.enactor` | dbref | dbref | The player or object that issued the command. |
| `ctx.cause` | dbref | dbref | The original MUX command cause. |
| `ctx.command` | string | string | The command text tested by the Lua pattern. |
| `ctx.scope` | `nil` | `"global"` | Present only for global commands. |
| `ctx.descriptor` | number or `nil` | number or `nil` | The fd of the descriptor that typed the command, when the command came from a live connection rather than a queued or scheduled execution. |
| `ctx.args` | empty table | empty table | Reserved for event arguments; command captures are passed as handler arguments instead. |

Use `ctx.enactor` for player-facing notifications. An object module may use
`ctx.object` with the [`mux` package](packages/mux/) to store persistent state.
Global handlers must not assume an object is present. Use `ctx.descriptor`
with [`mux.session.flow_start`](packages/mux/session/flow-start/)
to start an [interactive flow](flows/) on the connection that issued the
command.

## Discovering commands

Wizards can use `@list commands` to see three separate command groups:
built-in commands, global Lua commands, and object Lua commands. Each Lua entry
shows its pattern and source. Global entries use the module path as their
source; object entries use the object's name and dbref.

The list applies the invoking player's Lua command access level. Object entries
are further limited to command sources reachable from the player's current
location, including the player, inventory, room, room contents, and applicable
zones. Halted objects and objects blocked from the relevant command scope are
omitted. Listing does not evaluate patterns or run handlers.
