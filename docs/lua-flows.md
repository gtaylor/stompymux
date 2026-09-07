# Interactive Lua flows

Trusted game callbacks call `mux.session.flow_start(descriptor, module, first_step)`.
The function returns no value and synchronously runs the initial step. The target
must be an authenticated session without another active flow. Explicit targeting
is allowed from object/global callbacks and background work; it is not limited to
the caller's descriptor. No descriptor is inferred for background work.

Modules resolve within the calling game module's `global_logic` or `object_logic`
root, using the active source snapshot. Helper packages inherit that calling root.
Paths cannot escape the root, refer to another root or expand `require` access.
Modules expose a `flows` table mapping nonempty step names to functions.

```lua
return {
  commands = {{
    name = "confirm", permission = "everyone", pattern = "^confirm$",
    handler = function(ctx)
      mux.session.flow_start(ctx.descriptor, "confirmation.lua", "ask")
      return true
    end,
  }},
  flows = {
    ask = function(ctx)
      if ctx.input == nil then
        return {action = "repeat", prompt = "Proceed? (y/n) "}
      end
      if ctx.input == "y" then
        return {action = "done", message = "Confirmed."}
      end
      return {action = "cancel", message = "Cancelled."}
    end,
  },
}
```

Save this example as `global_logic/confirmation.lua` and reload Lua. The supplied
`flow_examples.lua` also demonstrates menus and multi-step scratch data.

## Steps and context

`ctx.scope` is `"flow"`. `enactor` and `cause` are the target player's dbref;
`descriptor` is the targeted session. There is no `object`. `ctx.input` is nil
on startup and immediate transitions; submitted lines preserve whitespace and
empty input. `ctx.flow` is a fresh table containing the prior scratch fields.

Return an outcome table:

- `repeat` (or omitted action): keep the step and show the prompt in bold.
  Missing prompt/message repeats the last prompt.
- `goto`: require `step`, optionally show a message, and immediately invoke the
  new step with nil input.
- `done` or `cancel`: optionally show a message and release the session.

`prompt` takes precedence over `message` for all actions. Neither gets an added
newline. Include `\r\n` explicitly when needed. Ordinary `pemit` messages retain
their existing behavior and stay ordered with private flow prompts.

## Limits and errors

Scratch is a flat store of at most 16 fields: string keys up to 31 bytes and
string values up to 8,191 bytes. Finite numbers are accepted and become strings
before the next callback. Binary strings are preserved, except embedded NUL.
Invalid types, excess fields and overlong strings reject the operation atomically.
Step names are limited to 63 bytes; prompt/message text to 8,191 bytes.

Each invocation shares the configured instruction, memory and output budgets
across its immediate steps. At most 32 immediate goto transitions and 32 nested
starts are allowed. Starts may target another session, but cannot replace an
active flow. Output uses the normal styled-text renderer and transport limits.

Startup raises structured errors recognized by `mux.error.is`: `mux.connection.invalid`,
`mux.connection.unavailable`, `mux.module.invalid`, `mux.unavailable.checking`, or
`mux.runtime`. Use `mux.error.codes.connection.invalid` (or the equivalent
`mux.error.code_tree("mux").connection.invalid`) for immutable typed codes. A Lua `pcall` catching a failed start cannot retain that start's world,
flow or output effects.

Flow startup is unavailable during module initialization, `@lua/check`, and an
unhosted test VM. Server-hosted `@lua/test` callbacks can explicitly target live
sessions from game-module callbacks. Their world changes follow the existing test
runner's per-invocation persistence rules; flow startup itself is atomic.

## Transactions and lifetime

A submitted line, scratch update and complete immediate chain form one transaction.
World mutations are validated and saved before prompts or new flow state publish.
Unchanged worlds require no database write. Persistence failure restores the prior
step, scratch and prompt for retry; a failed initial start leaves no active flow.
Script, validation or resource failures cancel the affected flow and discard its
transaction's mutations and output. Lua globals themselves are not rolled back.

Every input line is flow input before macros, channel aliases, native/Lua commands
and exit matching. Command quotas still apply. `quit` has no special meaning until
the flow ends; scripts should provide their own cancellation choice. There is no
new secret-input or echo-control API.

Flows belong to individual sessions. Disconnect, boot, purge, slow-client eviction
and shutdown discard them without running completion steps. Rollback cannot
resurrect detached sessions. Successful Lua reload carries plain scratch/step data
into the new VM; the next input resolves the current module table. Removed modules
or steps cancel on input. Failed reload leaves active flows intact. Server restart
loses flows; nothing is added to persistent game state or the database schema.
