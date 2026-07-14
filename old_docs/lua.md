# Lua scripting

LuaJIT modules live below `game.run/lua` in three separate roots:

```
lua/
  object_logic/      # @luaparent modules and their private helpers
  global_logic/      # globally matched command and scheduled modules
  packages/          # shared require-only helpers
```

Attach an object module with `@luaparent <object>=<relative-path>.lua`; the
path is relative to `object_logic`, and omitting it clears the attachment. For
example, `@luaparent #123=hello.lua` selects
`game.run/lua/object_logic/hello.lua`. Absolute paths, `..`, and paths into the
other roots are rejected. The closest attachment in the ordinary MUX parent
chain is active. Only wizards may attach or reload scripts.

`@luareload` atomically rebuilds the Lua state from every attached module,
every global-logic module, and their dependencies. If a file or dependency
fails to load, the current state remains active.

Use dotted names with `require`, such as `require("area.helper")`. An object
module searches `object_logic/area/helper.lua` before
`packages/area/helper.lua`; global logic does the same under
`global_logic`; a package may only require another package. Modules are
cached by their resolved root and path, so identically named private helpers do
not collide. Lua's native `package` table is not exposed.

Each module returns a table containing optional ordered `commands`, `events`,
and `schedules` tables. A command entry has a native Lua `pattern` and a
`handler(ctx, ...)`.
Returning `true` handles the command; `false` or `nil` allows legacy `$`
commands to run. Event keys are lower-case action attribute names, such as
`aenter`, `aleave`, `ahear`, `aconnect`, and `daily`. An attached module
suppresses that object's legacy action attributes, even when it lacks an event
function.

The `mux` API provides `attr_get(object, name)`, `attr_set(object, name,
value)`, `notify(object, message)`, and `command(command)`. Commands execute
as `#1` and are queued after the current handler. Lua has no filesystem,
process, debug, FFI, coroutine, or dynamic-loading APIs. The configured memory
cap applies to the complete state and each callback has an instruction cap.

## Global logic

Global logic files are recursively discovered below `global_logic` and
loaded in lexical relative-path order. Use domain-oriented paths such as
`player/help.lua`, `player/communication.lua`, `wizard/maintenance.lua`, and
`world/travel.lua`; add numeric path prefixes only for a deliberate
cross-domain priority. Each module uses the same `commands` table contract as
an object module. Shared parsing, formatting, and policy helpers belong in
`packages`.

Global command handlers run after every local or zone Lua and legacy `$` command has
declined the command, replacing the old master-room programmable-command
stage. The first global handler that returns `true` wins. Its context has
`ctx.scope == "global"` and `ctx.object == nil`, with the usual `enactor`,
`cause`, and `command` fields. Global modules do not receive event hooks.
Master-room exit behavior and other non-command uses of the master room are
unchanged.

Object and global logic modules can also provide named `schedules` entries with
numeric five-field UTC cron expressions. Object schedules run once for every
object that effectively inherits the Lua parent; global schedules run once per
matching module entry. Jobs receive deterministic 0--54 second jitter and do
not replay missed minutes. Inspect active schedules with the wizard-only
`@luaschedule [<object>|<path>.lua|global_logic/<path>.lua]` command.

## Starter examples

`game.run/lua/object_logic/example.lua` is a minimal hello-world command. Attach
it to an object, then enter `hello` while that object is in the normal
command-match scope:

```
@luaparent #123=example.lua
@luareload
```

`game.run/lua/object_logic/counter.lua` demonstrates durable state. Its `count`
command increments the attached object's `LuaCount` attribute, so the value
survives Lua reloads and server restarts.

`game.run/lua/object_logic/events/enter_notice.lua` demonstrates an `Aenter`
replacement:

```
@luaparent #456=events/enter_notice.lua
@luareload
```

Its `aenter` function is called whenever the existing action-attribute path
would run for that room. Attachments suppress the corresponding legacy action
attribute, so migrate the behavior before attaching the module.
