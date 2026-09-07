# Lua object APIs and runtime administration

`mux.world.types.ROOM`, `THING`, `EXIT` and `PLAYER` are immutable typed
constants. `object:type()`, `create_object` and type filters use these constants;
numbers, strings and flag/power constants are rejected.

```lua
local types = mux.world.types
local room = mux.world.create_object({type = types.ROOM, name = "Workshop"})
local thing = mux.world.create_object({
    type = types.THING, name = "Toolbox", location = room, home = room,
})
thing:set_zone(room)
for _, object in ipairs(mux.world.list_objects({
    types = {types.THING, types.PLAYER}, in_zone = room,
})) do
    assert(object:zone():dbref() == room:dbref())
end
```

`list_objects` returns ascending dbrefs, excludes Garbage and includes GOING.
Its optional `types` filter is a dense array: omitted/nil means all types and
`{}` means none. `in_zone` matches direct zone assignments. `object:contents`
uses the same typed filter and can also accept `visible_to`.

Objects expose `destination`, `home`, `location`, `zone`, `affiliation` and
`lua_parent` getters. Destination applies to exits; home/location apply to
players and things. Other relationships apply to all live object types.
Unset optional relationships return nil. Relationship results are object handles,
except `lua_parent`, which returns a relative module path or nil.

Corresponding `set_*` methods exist except for location. Arguments are required;
explicit nil clears destination, zone, affiliation or parent. Home cannot be
cleared. References accept object handles or integer dbrefs, require live objects,
and reject GOING receivers/targets when mutating. Homes and exit destinations
must be rooms, players or things; homes also reject self/descendant containment.
Zones must be rooms or things; affiliations can reference any live object.

These trusted setters do not invoke locks or movement callbacks. `set_zone`
does not strip flags or powers. Use the movement API to relocate objects.
`set_lua_parent` accepts only a safe `.lua` path in the active object-module
catalog; nil clears it. Runtime table edits do not register new code.

`object:flags():list()` and `object:powers():list()` return typed constants in
catalog order, alongside existing `has/add/remove` methods. All durable mutations
participate in callback/command transactions. Failure discards staged output and
restores world state while preserving accurate session-owned CONNECTED flags.

## Wizard administration

- `@lua/parent <object>=<path>` attaches an active module; omitting/emptying the
  right-hand side clears it. Wizards may edit other Wizards, including GOD.
- `@lua/viewparent <#dbref|path>` reads current disk source and displays it
  literally in private bounded chunks. Viewing does not activate edited code.
- `@lua/check` builds an isolated validation VM. World, session, channel and state
  operations are unavailable; pure text/configuration helpers and constants work.
- `@lua/reload` builds a candidate live VM from a new source snapshot. It validates
  and persists initialization mutations before replacing active code or sending
  output. Failure preserves the old runtime, world and queued schedules.
- `@lua/schedule` inspects captured registrations without executing them.

Filesystem reads run on blocking workers; Lua execution remains on the serialized
world owner. Source discovery is lexical and bounded by the Lua memory setting.
Parent paths cannot be absolute, contain traversal components or escape the Lua
root through symlinks. Helper packages are captured in the same snapshot; `require`
uses captured sources, not subsequent disk edits. Unused packages are syntax-checked.
Modules validate their command, event, message, appearance, lock and schedule shapes.

Reload discovers added files and removes deleted modules, but fails if an attached
parent is missing. Module globals reset. Initialization may mutate candidate world
state; bootstrap/startup/connect hooks are not rerun. Sessions and their terminal
state, help index, macros and command cursors remain intact.

Successful reload cancels old scheduled jobs, retaining the minute high-water
mark. It does not recollect the current minute or backfill missed minutes. Failed
reload retains pending jobs. See [scheduling](lua-schedules.md).

Checking is deliberately stricter than live initialization: code that writes state
at module load can reload but cannot pass `/check`. Checking includes test suite
declarations, but never executes their hooks or test bodies.

`@lua/test [filter]` runs suites in a separate VM with live-world services. Use
`/unit`, `/integration` and `/verbose` modifiers. Test mutations survive assertion
and runtime errors; validation or persistence failure rolls back only that
invocation. Active game code, globals and scheduled jobs remain unchanged.
Structured testing errors expose `mux.error.new`, `wrap`, `is` and immutable
`code_tree("testing")` codes for the unchanged `testing` package. This does not
convert other native errors to structured codes. Interactive flows and destruction
APIs remain outside this interface.

## Message routing

`mux.world.pemit(object, message)` uses the same bounded direct notification policy
as the C binding. It can relay through AUDIBLE exits attached to the target; it
does not implement the absent listener-only downward branch. Strings retain legacy
styles; immutable Markdown documents retain their format when forwarding prefixes
are added. Invalid objects, embedded NULs and output-budget failures raise errors.
Fan-out is staged atomically even when a caller catches the Lua error with `pcall`.

## Commands executed from a queue

`@force` and `@wait` dispatch ordinary Lua commands using the executing object's
current permissions and module registrations. `ctx.enactor` is the executor;
`ctx.cause` identifies the forcing actor, or the cause retained by a nested wait.
`ctx.object` continues to identify the command's scoped object and
`ctx.descriptor` is nil. Native action, movement and lock callbacks retain this
causal context without granting the cause's authority to the executor.

Queued commands receive fresh callback budgets. Each command commits its world
changes before delivering staged output. Failure discards that command's changes
and output, but subsequent commands may still execute. Queues are runtime-only
and survive Lua reloads; there is no Lua queue API in this tranche. Session-only
APIs continue to require a real descriptor.
