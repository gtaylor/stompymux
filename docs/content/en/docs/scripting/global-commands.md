---
title: Global logic
linkTitle: Global logic
type: docs
weight: 20
---

Global Lua logic modules live under `game/lua/global_logic`. Every
`.lua` file in that tree is discovered recursively during startup and
`@lua/reload`. Files are loaded in lexical relative-path order, so use
domain-oriented paths such as `player/help.lua`, `world/travel.lua`, and
`wizard/maintenance.lua`. Use numeric prefixes only when deliberate
cross-domain priority is necessary.
Use [`@lua/check`](validating-and-reloading/) to validate every Lua module before reloading.

Each global module returns a table containing `commands`:

```lua
return {
  commands = {
    {
      pattern = "^global%-hello$",
      handler = function(ctx)
        mux.notify(ctx.enactor, "Hello, world, from a global Lua command!")
        return true
      end,
    },
  },
}
```

Global handlers run after local and zone Lua command matching declines the
command. Master-room exits and other non-command behavior are unchanged.
Matching stops at the first global handler that returns `true`; `false` or
`nil` allows the next handler to try.

Global contexts include `ctx.scope == "global"`, `ctx.enactor`, `ctx.cause`,
and `ctx.command`; `ctx.object` is `nil`. Global modules receive command
dispatch only, not object event hooks.

See [Commands](commands/) for Lua-pattern syntax, capture arguments, and the
handler context table.

Global modules resolve `require("name")` in `global_logic` before the
shared `packages` root. Put shared parsing, formatting, and policy helpers in
`packages`. The working `game/lua/global_logic/example.lua` module
defines the `global-hello` command.

Global logic modules may also define `schedules`, an ordered array of named
UTC five-field cron handlers. A global schedule runs once per matching module
entry and receives `ctx.scope == "global"`; `ctx.object`, `ctx.enactor`, and
`ctx.cause` are `nil`.
