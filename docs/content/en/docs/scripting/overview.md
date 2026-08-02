---
title: Overview
linkTitle: Overview
type: docs
weight: 1
---

LuaJIT modules live under `game/lua` in three separate roots:

```text
lua/
  object_logic/      # @lua/parent modules and their private helpers
  global_logic/      # globally matched command and scheduled modules
  packages/          # shared require-only helpers
```

## Object and global logic

Attach a module to an object with the wizard-only `@lua/parent
<object>=<path>.lua`; the path is relative to `object_logic`, and omitting it
clears the attachment. Each object uses only its own direct attachment. See
[Object scripting](scripting-objects/) for the
full module contract, the native event catalog, and how load errors are
handled.

Global logic files are discovered recursively below `global_logic` and
loaded in lexical relative-path order; use domain-oriented paths such as
`player/help.lua` and `world/travel.lua`. Global command handlers run only
after every local or zone Lua command has declined the command. See
[Global logic](global-commands/) for details.

## Module contract

Each module returns a table with optional `commands`, `schedules`, and `flows`
entries; object modules may also provide `events`, `locks`, successful action
`messages`, and appearance functions. A command entry pairs a native Lua
`pattern` with a `handler(ctx, ...)` and may set `access` to `"wizard"` or
`"god"`; omitted access is public. Returning `true` handles the command,
`false` or `nil` lets other matching continue. See [Commands](commands/) for
pattern syntax, access behavior, and the handler context table.

A module's `flows` table holds named step functions that
[`mux.flow_start`](packages/mux/#muxflow_startdescriptor-module-first_step)
can drive as a multi-step conversation on a connected player's own
descriptor - the interactive counterpart to `commands` for menus, prompts,
and confirmations. See [Interactive flows](flows/).

Object and global modules can also declare `schedules`: named entries with
five-field UTC cron expressions. Object schedules run once for every directly
attached object; global schedules run once per
matching module entry. Scheduled jobs receive deterministic jitter and do not
replay missed minutes. Inspect active schedules with the wizard-only
`@lua/schedule` command.

## Imports

Use dotted names with `require`, such as `require("area.helper")`. An object
module searches `object_logic/area/helper.lua` before
`packages/area/helper.lua`; global logic does the same under
`global_logic`; a package may only require another package. Modules are
cached by their resolved root and path, so identically named private helpers
in different roots do not collide. Lua's native `package` table is not
exposed.

## The `mux` API

The `mux` table is the server interface exposed to Lua modules. Use
`mux.object(dbref)` for object properties, containment, locks, and typed
persistent state. Styled text, notifications, queued commands, connection
summaries, and interactive flows remain top-level `mux` operations. Queued
commands execute as `#1` after the current handler completes. See the
[`mux` package reference](packages/mux/) for the full API.

Lua has no filesystem, process, debug, FFI, coroutine, or dynamic-loading
APIs. The configured memory cap applies to the complete Lua state. Persistent
object state has separate per-value and per-object limits.

Native control is role-only: God controls everything; Wizards control every
non-Wizard object and player but cannot control themselves, God, or another
Wizard; mortals control nothing, including themselves. Zones do not affect
control. Lua is trusted and uses the `mux` API, including commands queued as
`#1`, to manipulate any object.

## Validating and reloading

Use the wizard-only `@lua/check` to verify every module before putting
changes into service, then `@lua/reload` to atomically rebuild the Lua state
from every attached module, every global logic module, and their
dependencies. If a file or dependency fails to load, the current state
remains active. See [Validating and reloading](validating-and-reloading/).

## Starter examples

`game/lua/object_logic/example.lua` is a minimal hello-world command. Attach
it to an object, then enter `hello` while that object is in the normal
command-match scope:

```text
@lua/parent #123=example.lua
@lua/reload
```

`game/lua/object_logic/counter.lua` demonstrates typed durable state. Its
`count` command increments the attached object's `counter/count` state value,
which survives Lua reloads and server restarts.

`game/lua/object_logic/events/enter_notice.lua` demonstrates an `on_enter`
handler:

```text
@lua/parent #456=events/enter_notice.lua
@lua/reload
```

Its `on_enter` function runs whenever the room receives the native enter event.

`game/lua/global_logic/example.lua` defines the working `global-hello`
global command.

`game/lua/global_logic/who.lua` defines the player-facing `who` command.
Wizards use the built-in `@who` command when they need privileged connection
details, `@session` for per-client queue and traffic counters, and `@telnet`
for negotiated Telnet state and NEW-ENVIRON values.

`game/lua/global_logic/flow_examples.lua` demonstrates
[interactive flows](flows/): `flow-demo confirm`, `flow-demo menu`, and
`flow-demo signup`.
