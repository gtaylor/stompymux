---
title: Commands
linkTitle: Commands
type: docs
weight: 15
---

Lua command modules return a `commands` array. Each entry has a native Lua
`pattern` and a `handler` function:

```lua
return {
  commands = {
    {
      pattern = "^say%s+(.+)$",
      handler = function(ctx, message)
        mux.notify(ctx.enactor, "You said: " .. message)
        return true
      end,
    },
  },
}
```

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

The pattern is matched against the otherwise-unmatched command text without
softcode evaluation. Each capture becomes an argument after `ctx`, in capture
order. When a pattern has no explicit captures, Lua passes the complete match
as the argument after `ctx`.

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
handler handles the command, the normal local and zone legacy `$` command
matching continues. A Lua pattern or handler error is logged and counts as
handled, preventing duplicate legacy side effects.

Global command modules run only after local and zone Lua and legacy matching
decline the command. Their modules are checked in lexical path order and stop
at the first handler that returns `true`.

## Command context

Every command handler receives a context table as its first argument.

| Field | Local object command | Global command | Description |
| --- | --- | --- | --- |
| `ctx.object` | dbref | `nil` | The command-scope object whose active module handled the command. |
| `ctx.enactor` | dbref | dbref | The player or object that issued the command. |
| `ctx.cause` | dbref | dbref | The original MUX command cause. |
| `ctx.command` | string | string | The command text tested by the Lua pattern. |
| `ctx.scope` | `nil` | `"global"` | Present only for global commands. |
| `ctx.args` | empty table | empty table | Reserved for event arguments; command captures are passed as handler arguments instead. |

Use `ctx.enactor` for player-facing notifications. An object module may use
`ctx.object` with the [`mux` package](packages/mux/) to store persistent state.
Global handlers must not assume an object is present.
