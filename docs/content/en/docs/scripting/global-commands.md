---
title: Global commands
linkTitle: Global commands
type: docs
weight: 20
---

Global Lua command modules live under `game.run/lua/global_commands`. Every
`.lua` file in that tree is discovered recursively during startup and
`@luareload`. Files are loaded in lexical relative-path order, so use
domain-oriented paths such as `player/help.lua`, `world/travel.lua`, and
`wizard/maintenance.lua`. Use numeric prefixes only when deliberate
cross-domain priority is necessary.
Use [`@luacheck`](lua-check/) to validate every Lua module before reloading.

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

Global handlers run after local and zone Lua and legacy `$` command matching
decline the command. They replace the master-room programmable-command stage;
master-room exits and other non-command behavior are unchanged. Matching stops
at the first global handler that returns `true`; `false` or `nil` allows the
next handler to try.

Global contexts include `ctx.scope == "global"`, `ctx.enactor`, `ctx.cause`,
and `ctx.command`; `ctx.object` is `nil`. Global modules receive command
dispatch only, not object event hooks.

See [Commands](commands/) for Lua-pattern syntax, capture arguments, and the
handler context table.

Global modules resolve `require("name")` in `global_commands` before the
shared `packages` root. Put shared parsing, formatting, and policy helpers in
`packages`. The working `game.run/lua/global_commands/example.lua` module
defines the `global-hello` command.
