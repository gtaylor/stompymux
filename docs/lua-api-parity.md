# Lua API contracts

The C bindings under `btmux-khi/src/mux/lua/packages/mux` define the non-BattleTech
API. The checked-in [callable inventory](../tests/fixtures/lua-api.json) records
functions and methods with their C source references. Integration tests resolve
every entry against initialized bindings. Existing package-specific tests cover
flags/powers, state, communication, text, flows, testing and configuration.

## Packages and identities

`require("mux")` returns the global `mux` namespace. Built-in bindings initialize
before editable game modules. The packages are `world`, `session`, `config`,
`text`, `comsys`, `telnet` and `error`; `log` and `check_db` are top-level functions.
No additional `require` interface or BattleTech operations are installed.

Objects are immutable userdata identified by world, dbref and incarnation.
`mux.world.object(dbref_or_object)` creates a handle; `:dbref()` retrieves its
identity. A table with an `_id` field is not an object. Two handles for the same
live incarnation compare equal. A handle retained across rolled-back creation
cannot address a later object allocated at that provisional dbref. Object, state,
flag, power and channel handles reject stale identities after destruction.

Typed catalogs remain immutable: `world.types`, `world.flags`, `world.powers`,
`world.locks` and `comsys.flags`. Constants from different catalogs are not
interchangeable. Command/configuration aliases do not change Lua constant names.
Host closures, module lookup tables and connection snapshots are private. Returned
session tables are fresh values; editing them cannot change connection state.

## Structured errors

```lua
local errors = mux.error.namespace("mygame", {"access.denied", "input.invalid"})
local ok, result = mux.error.pcall(function()
    mux.error.raise(errors.access.denied, "Access denied", {door = 13})
end)
assert(not ok and result:is("mygame.access"))
local outer = mux.error.wrap(result, "mygame.command", "Command failed")
assert(outer:root() == result)
```

`new{code, message, detail?, cause?}` constructs a value; `raise` throws one.
`is(value, code)` and `error:is(code)` use exact or dotted-prefix matches.
`check(value, failure)` returns a truthy value unchanged or raises the supplied
failure unchanged. `wrap` preserves structured causes and normalizes other errors
as `mux.runtime`. `root` follows table causes with bounded, cycle-safe traversal.
`mux.error.pcall` retains all successful return values, including intervening nils;
failures retain structured table identity and receive traceback information.

`mux.error.codes` is `code_tree("mux")`. The other native roots are `testing` and
`btech`; the latter exposes inert error symbols without implementing its APIs.
Custom namespaces use dotted lowercase segments and cannot use those roots.
Code nodes support `.code`, child lookup, equality and string conversion. Unknown
symbols and invalid namespace declarations fail explicitly.

Domain failures travel as typed host errors and become structured Lua values at
binding/protected-call boundaries. Ordinary `pcall` and `xpcall` can inspect their
codes. Arbitrary Lua error values remain intact; ordinary argument conversion/type
errors remain ordinary errors. Error messages include context, but callers should
branch on codes rather than parse messages. The complete native code catalog is
in `src/lua/packages/error/catalog.rs`.

## Movement, destruction and repair

`mux.world.teleport_object{object=..., destination=...}` uses shared movement
policy, containment validation, locks and object callbacks with GOD as cause and no
invented descriptor. Location transition providers/events use C cause `#-1`.
Teleport-source and leave actions precede relocation; appearance precedes
teleport/move and enter actions. DARK teleports retain silent location providers
and move events, while suppressing teleport and location events. Only players and things can move; destinations must be valid
containers. A same-location move is a no-op. Denial raises an error, and callback
failure restores movement and staged effects even when caught with `pcall`.

Native generic movement also renders immediately after relocation. Exit traversal
runs exit success/on_success, source leave/on_leave and destination enter_source,
relocation/appearance, exit drop/on_drop, traveler move/on_move, destination
enter/on_enter and source leave_destination. Exit actions use `operation="traverse"`;
traveler/location actions use `"move"`, retaining the original command cause.
That cause retention is an approved Rust difference: C's normal exit/enter/leave
command entrypoints supply `#-1` to movement actions (see parity finding M05).
Providers still execute when silent, with direct messages retained and neighbor
messages/events suppressed according to the action policy. Shared action neighbor
text uses the bounded notification graph, including AUDIBLE forwarding.

`mux.world.destroy_object(object, {override=true})` silently schedules normal
GOING destruction. Options are optional; `override` bypasses SAFE, not protection
of foundational objects or Wizard players. Scheduling does not immediately purge.

`mux.check_db()` performs semantic repair synchronously, using the same planner
as `@dbck`. Later Lua statements immediately see repaired objects and tombstones.
Relocation callbacks execute in that boundary. Owned-record cleanup, SQL integrity
checks and disconnects happen only after the outer world transaction commits.
A failed save restores the prior world and discards messages, logs, flows and
maintenance effects. Multiple checks retain pending cleanup until commit.

Live test invocations keep their established semantics: valid mutations can commit
even after an assertion/runtime error; invalid or unsaved invocations roll back.
Checking VMs permit pure configuration/text/error helpers and constants, and raise
`mux.unavailable.checking` for live services. State enumeration/mutation, repair,
teleportation, destruction and staged logging require an active transaction.

## Telnet environment

```lua
local term = mux.telnet.environment_get(ctx.descriptor, "var", "TERM")
local enabled = mux.telnet.environment_has(ctx.descriptor, "uservar", "CLIENT_FEATURE")
```

Both calls require a live session ID, including a live unauthenticated connection
when explicitly supplied. Namespace names are exactly `var` or `uservar`.
Names and values are byte-preserving Lua strings. `get` returns nil for an absent
entry; `has` distinguishes absence from an empty value. These read-only lookups
use the latest host snapshot and cannot alter negotiation or persistent state.

## Intentional Rust behavior

- Markdown documents and effective/defaulted TOML configuration lookup remain
  supported extensions. Strings keep existing bracket-markup behavior.
- Existing configured instruction, memory, state and output budgets apply.
- World/output/flow/maintenance effects participate in nested rollback; Lua file
  logs are submitted only after commit. Logging-only calls do not write the DB.
- CONNECTED remains session-owned and is never writable through flags or snapshots.
- Native Lua services use shared domain logic and asynchronous outer persistence;
  they do not call SQLite synchronously from the VM.
- No BattleTech package, C internals, host filesystem access or extra debug/native
  module loading is exposed.

`mux.text.is_printable_ascii(value)` requires an actual Lua string and raises an
argument error for all other types. It checks bytes `0x20–0x7e` without UTF-8
conversion; empty strings return true, and embedded NUL/non-ASCII bytes return false.
