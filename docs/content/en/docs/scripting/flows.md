---
title: Interactive flows
linkTitle: Flows
type: docs
weight: 25
---

Softcode has no interactive-input primitive; Lua does, through `mux.flow_start`
and a module's `flows` table. A flow drives a connected player's own
descriptor through a menu, a confirmation, or a multi-step form, one line of
input at a time, without the player prefixing every line with a command.

```lua
return {
  commands = {
    {
      pattern = "^delete%-character$",
      handler = function(ctx)
        mux.flow_start(ctx.descriptor, "confirm_delete.lua", "confirm")
        return true
      end,
    },
  },
  flows = {
    confirm = function(ctx)
      if ctx.input == nil then
        return { action = "wait", prompt = "Delete this? (y/n) " }
      end
      if ctx.input:match("^[Yy]") then
        return { action = "done", message = "Deleted." }
      end
      return { action = "cancel", message = "Cancelled." }
    end,
  },
}
```

## Starting a flow

`mux.flow_start(descriptor, module, first_step)` attaches a flow to the given
descriptor and immediately shows its first prompt. `descriptor` is
`ctx.descriptor` from the command or event that is starting the flow -
flows are always driven from a live connection, never from a queued or
scheduled context, so `ctx.descriptor` is only present when one exists (see
[Commands](commands/)). `module` is resolved the same way `require` resolves
a name: relative to the calling module's own root, without repeating the root
name. Raises a Lua error if the descriptor doesn't exist, already has a flow
running, or the module has no such flow step.

While a flow is active, every line the player sends goes to the flow - there
is no escape hatch back to ordinary commands. A flow step is responsible for
its own way out (a `cancel` action, or a dedicated keyword the step itself
recognizes).

## Flow steps

Each entry in a module's `flows` table is a plain function keyed by step
name, called once to prime its prompt and again for every line of input
submitted while that step is current:

```lua
flows = {
  step_name = function(ctx)
    if ctx.input == nil then
      -- Just became current (flow start, or after a "goto" into it).
      return { action = "wait", prompt = "..." }
    end
    -- ctx.input is the line the player just sent.
    return { action = "done" }
  end,
}
```

A step returns a table describing what happens next:

| Field | Meaning |
| --- | --- |
| `action` | One of `"wait"`, `"goto"`, `"done"`, or `"cancel"`. |
| `step` | Required with `"goto"`: the next step name in the same `flows` table. |
| `prompt` | Text to show. With `"wait"`, omitting it repeats the current prompt. |
| `message` | A one-shot message shown before a `"goto"`, `"done"`, or `"cancel"` teardown. |

`"wait"` stays on the current step and (re)shows its prompt - use this to
reject invalid input without losing progress. `"goto"` moves to a named step
in the same table, priming it immediately (so a chain of steps with no
intervening input runs to completion, or until an internal safety limit
cancels a runaway loop). `"done"` and `"cancel"` both end the flow; `"done"`
is normal completion and `"cancel"` is an aborted one, but the engine treats
them identically otherwise.

## Cross-step state

`ctx.flow` is a table that survives from one step call to the next - stash
data on it as the player answers each prompt:

```lua
flows = {
  ask_name = function(ctx)
    if ctx.input == nil then
      return { action = "wait", prompt = "Name? " }
    end
    ctx.flow.name = ctx.input
    return { action = "goto", step = "ask_confirm" }
  end,
  ask_confirm = function(ctx)
    if ctx.input == nil then
      return { action = "wait", prompt = "Confirm " .. ctx.flow.name .. "? (y/n) " }
    end
    -- ...
  end,
}
```

Only string and number values round-trip; anything else assigned to
`ctx.flow` (a table, a function, a boolean) is dropped with a logged warning.
This is a deliberate trade-off: `ctx.flow` is backed by a small store of plain
values on the descriptor, not a reference into the Lua state, specifically so
a flow survives `@luareload` rebuilding the entire state out from under it. A
step that no longer exists after a reload still fails the same way any other
removed API would.

## Context

A flow step receives the same kind of context table as a command handler
(see [Commands](commands/)), with two differences: `ctx.scope` is `"flow"`,
and two extra fields are always present:

| Field | Description |
| --- | --- |
| `ctx.input` | `nil` while priming a step's prompt, otherwise the submitted line. |
| `ctx.flow` | The cross-step scratch table described above. |

## Example

`game/lua/global_logic/flow_examples.lua` has three complete, working
examples reachable with `flow-demo confirm`, `flow-demo menu`, and
`flow-demo signup`: a single-step confirmation, a numbered menu using
`"goto"`, and a multi-step form that threads data through `ctx.flow`.
