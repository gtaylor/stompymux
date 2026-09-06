# StompyMUX Rust foundation

An independent Rust implementation of the MUX foundation. The copied game world,
configuration and Lua modules are the compatibility fixtures; no C server code is
linked or invoked. LuaJIT and SQLite are built from vendored dependencies.

## Run the copied world

From this directory, with Rust and a C compiler installed:

```sh
cargo run -- serve --game-dir game
```

The server reads and updates `game/data/stompymux.db` directly. Existing
`stompymux-rs.db` JSON snapshots are left untouched and are not merged or loaded.
Use only one server writer for a database; do not run legacy and Rust servers
against the same file simultaneously.

The supplied configuration listens on `127.0.0.1:5555`. Without a configured
port, the compiled legacy default is 6250.
Override with `--listen-address 0.0.0.0 --port 5556` when appropriate. Connect
using a Telnet/MUD client. Enter an existing player name, alias, or dbref (such
as `#2`) and password, or enter
a new name and follow the registration prompts. Registration connects the new
player immediately. Passwords are hidden using Telnet ECHO negotiation; this
milestone uses plain TCP and does not provide transport encryption.

Players can use `look`/`l`, `say <message>`/`"<message>`, `WHO`, exit names or
aliases, `global-hello`, and `quit`. The starter-room exit remains wizard-only.
Accounts and locations survive restart. Multiple sessions are supported;
connection hooks receive `reconnect`, and disconnect hooks run only for the
last session. Ctrl-C and SIGTERM stop the listener and finish accepted writes.

## Storage

The live format is **schema 32**. The supplied fixture has 16 objects, two
accounts and two channels. Supported fields are read directly from the relational
tables. SQLx updates only changed supported columns in a single transaction;
existing rows are never replaced. Failed writes restore the in-memory world and
discard pending success output.

Unknown columns, deferred BattleTech/channel/macros/page-recipient data, indexes
and triggers remain in place. Unchanged Lua values retain their original SQLite
storage types, including UTF-8 blobs. Explicit state-key removals and expired
login-history entries remove only those owned rows. Unsupported required columns
on new rows cause an atomic failure instead of invented defaults.

Legacy containment lists (`contents`, `exits`, `next`) are updated for movement
and creation, preserving unaffected lists and the relative order of retained
members. Login history keeps at most four successes and three failures, newest
first within each outcome, additionally respecting a smaller configured total
limit. Lifetime login counters are independent of retained history.

Persistence uses SQLx 0.9 with Tokio and bundled SQLite, operation-scoped
connections and `database.busy_timeout_ms`. Database loads use read-only connections.
Normal writes do not change journal mode, foreign-key policy or schema metadata.
Older schemas and Rust JSON snapshots are rejected without conversion.

For a **new game**, use a separate game-directory copy with the configured live
database absent. `serve` exclusively creates schema-32 storage with mode 0600,
runs bootstrap Lua, and commits initialization atomically. Empty or malformed
existing files are rejected. Random administrator credentials are written to
`bootstrap-credentials.txt` with mode 0600; stale credentials are not overwritten.
Existing worlds never run first-startup hooks.

## Configuration

All 182 legacy TOML mappings are typed and retained, including options for
features not yet implemented. Twenty-one additional settings configure the Rust
runtime. See [configuration semantics and runtime defaults](docs/configuration.md)
and the annotated `game/stompymux.toml`.

Listener precedence is CLI → TOML → centralized defaults. Both IPv4 and IPv6
addresses are supported. Content paths resolve relative to `--game-dir` (default
`game`); recursive include paths resolve relative to the including file.

`database.game_database` names live schema-32 storage, defaulting to
`data/stompymux.db`. `database.legacy_game_database` is deprecated and ignored.
Changing configuration never moves files or merges data from another database.

## Wizard movement

Wizards and GOD can use `home` to return to their stored home, or
`@teleport <destination>` to move inside a room, player or thing.
`@teleport <object>=<destination>` moves another player, thing or exit; configured
aliases such as `@tel` work too. Ordinary players cannot use these commands.
Wizards may teleport other Wizards. Use dbrefs for remote targets, or `me`,
`here` and exact visible nearby names. Ambiguous names are rejected.

Containment cycles and invalid destinations are rejected. Moving an occupied
container carries its contents; relocating an exit preserves its linked
destination. Teleporting through exits and command switches remain unavailable.
Teleportation checks the destination's `teleport` lock and each enclosing
source container's `teleport_out` lock; errors deny movement. `home` bypasses
these locks, but still validates its stored destination.

Movement calls source `on_exit` and destination `on_enter` hooks with the moved
object as `enactor`, initiator as `cause`, immediate `source` and `destination`,
and the location hosting the callback as `object`. A descriptor is supplied
only when the moved player initiated the command. Moving an exit or remaining
in the same location does not fire occupant transition hooks. Containers without
an appearance callback use the generic Lua renderer for `look` and arrival.
Connected moved players receive the appearance on all their sessions.

Location changes and callback effects persist together before success output.
Failures roll back the move and discard its pending messages. No home fallback
is selected when the stored home is missing or invalid.

## Object flags

Objects use the shared 19-flag MUX catalog. Wizards can inspect flags with
`@list flags` and `@examine <target>` (including configured aliases such as
`@ex`). This examination command currently shows identity, type, flags and powers.
Use `@flag <target>=DARK` to set a flag and `@flag <target>=!DARK` to clear it.
Targets support `me`, `here`, dbrefs and exact visible nearby names or exit
aliases. Ambiguous names are rejected. Flag names and configured aliases are
case-insensitive; one flag is changed per command.

GOD controls all live objects. Wizards control themselves and non-Wizard
objects, but cannot edit another Wizard or grant Wizard status. Only GOD can
change WIZARD, and cannot clear its own WIZARD flag. GOING follows the legacy
special clearing policy; destruction itself remains deferred. Ordinary players
cannot use these administrative commands. Successful durable changes are saved
before acknowledgement.

**CONNECTED is session-owned.** It becomes true after a successful login or
registration and stays true until the last session disconnects. Lua lifecycle
hooks observe the updated value. Callback errors and database failures cannot
restore stale connection state. CONNECTED is stored as zero and
ignored in stored/default object flags; commands and Lua cannot override it.

Lua uses immutable constants such as `mux.world.flags.DARK` and
`object:flags():has/add/remove`. Mutation returns whether the flag changed;
unknown names and raw strings are rejected. Trusted Lua retains GOD-level
mutation authority, subject to GOD's WIZARD protection and session-owned
CONNECTED. Flags for deferred systems remain available as data without enabling
those systems.

## Object powers

The current legacy catalog contains one power, `IDLE`. It is persistent metadata
only: granting it does **not** bypass idle timeouts yet. New objects have no
powers, and imported powers survive restart.

Wizards can use `@power <target>=idle`, `@power <target>=!idle`, and `@list powers`.
`@examine` also shows a `Powers:` line. Target resolution and control permissions
match flag administration: GOD controls all live objects; Wizards control
themselves and non-Wizard objects. Names are case-insensitive, and each command
changes one power. Configured command aliases apply; power aliases are not added.
Changes are saved before success is reported.

Lua exposes immutable `mux.world.powers.IDLE` and
`object:powers():has/add/remove`. Add/remove return whether the set changed.
Use power constants rather than strings or flag constants. Trusted Lua can
change powers on any live object, with the same transactional rollback used by
other callback mutations.

## Implementation boundaries

- Tokio handles sockets and timers. A single owner serializes world and Lua
  work, using bounded channels. SQLx provides async SQLite access through its
  own worker threads; Argon2id runs on bounded blocking workers. Lua values
  never cross threads. A database commit serializes command processing while
  networking continues.
- LuaJIT runs the copied packages, object parents and global modules in lexical
  path order. World/state/flag handles, appearance and traversal callbacks,
  lifecycle hooks, local/global command matching, basic channel creation,
  configuration lookups, text formatting and WHO session APIs are available.
- Lua memory and persistent-state limits are enforced. Callback execution has
  a one-million-instruction budget. LuaJIT tracing is disabled so traces cannot
  bypass that budget; the LuaJIT runtime and language remain in use. OS, FFI,
  debugging and native module loading are unavailable to game scripts.
- Lifecycle callback failures are isolated; startup errors prevent listening.
  Failed commands roll back mutations. Invalid traversal policies deny entry.
  Lua output is bounded independently of Lua heap allocations.
- Telnet supports fragmented framing, ECHO, TTYPE, NAWS and UTF-8 CHARSET,
  with plain/16-color ANSI output. Unsupported options are declined. Input
  lines are bounded at 8 KiB; output messages at 64 KiB, with 128 queued
  messages per connection. Slow clients are disconnected. Login throttles,
  hash concurrency/rate limits, command quotas and idle timeouts are active.

BattleTech simulation, builder commands, player channel commands, cron and
interactive Lua flows, extended styled text/custom palettes, MCCP2, GMCP and
OSC 8 features remain deferred. Demo modules are copied unchanged: schedules
produce startup warnings, flows report an explicit unavailable-feature error,
and clickable markup renders visible text. Legacy help/type files describe a
larger API than this milestone implements. Deferred configuration is reported in one capability diagnostic. Site/access
rules parse and pass configuration checks, but block serving before writes or
listening because enforcement is not implemented.

## Validation

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tests use `tests/fixtures/game` copied into temporary directories, ephemeral
TCP ports and controlled credentials. They cover relational data preservation and schema
rejection, bootstrap idempotence, Lua limits/locks, real registration and login,
duplicate registration, password mismatch, throttling, speech, WHO, movement,
multiple sessions, restart durability, injected write failures, stale auth
results, bounded output and graceful shutdown. They do not mutate `game/` or
`../btmux-khi/`.

The original game assets and data were copied intact. Their inherited license
is retained in `LEGACY-LICENSE.md`.

## Object search

Wizards and GOD can use `@find [name][,low[,high]]` (also `@fi` or `@fin`).
Names match case-insensitive prefixes at the start of a name or word; wildcard
characters are literal. Bounds are inclusive, accept optional `#`, and default
to zero and the current database maximum when omitted or invalid.
Searches list controlled live objects except exits, ordered by dbref, including
DARK and GOING objects. Wizards cannot list GOD or other Wizards; GOD can list all.
Results use `Name(#dbref:type-and-flag-letters)`.

Each connection has its own search. Use `@find/next` to continue; a new search
replaces the previous one. Other commands leave it intact. The final page ends
with `***End of List***`. Each page rechecks current objects and permissions up
to the original upper bound. Searches never write the database or run callbacks.
`runtime.find_page_size` defaults to 20. The output byte limit may shorten pages
or truncate displayed names while preserving identity and flags. If a row and
footer cannot fit, the search remains available for retry.

## Command registration

Native and Lua commands share an enumerable registry of names, permissions,
matchers and sources. Native definitions in `src/commands/registry.rs` register
function pointers; command handlers live separately from dispatch and target
matching. Permission bits are `EVERYONE`, `WIZARD` and `GOD`; combining restricted
bits requires GOD. Object control, flag policies and movement locks still apply.

Lua declarations now require explicit metadata:

```lua
return {
  commands = {
    {
      name = "greet",
      permission = "everyone", -- also "wizard" or "god"
      pattern = "^greet%s+(.*)$",
      handler = function(ctx, name)
        mux.world.pemit(ctx.enactor, "Hello, " .. name)
        return true
      end,
    },
  },
}
```

Missing metadata or malformed patterns fail module loading with the source and
command index. Names are single tokens without `/`; the registry normalizes
names to lowercase. Lua patterns retain their original case behavior. Patterns,
permissions and handlers are captured at load time; editing the returned module
table does not change registration.

Configured aliases work for both native and Lua commands, preserving arguments.
Full-token aliases take precedence, followed by base-token aliases for switches;
there is no recursive alias expansion. Unaliased Lua commands see original input.

Native commands run first and reject unauthorized access or unsupported switches
without falling through. Lua tries eligible local objects by dbref, then global
modules in lexical order, preserving declaration order within modules. Restricted
Lua entries are skipped; a handler returning false/nil allows later handlers and
then exits to match. NO_COMMAND and HALTED still exclude object-local commands.
The metadata API prepares for `@list commands`; that command is not added yet.

## Shutdown and database maintenance

`@shutdown` and `@dbck` require Wizard status (or GOD), honor configured command
aliases, and accept no arguments or switches. `@dump` is not implemented: ordinary
mutations already commit to SQLite before success is reported.

`@shutdown` validates and saves first. A failed initial save cancels the command
and leaves sessions online. Once accepted, it broadcasts `Game: Shutdown by <name>`
and stops accepting connections, commands and authentication results, including
remaining commands in the same input batch. SIGINT and SIGTERM enter this same
shutdown coordinator, but proceed with best-effort cleanup if the initial save
fails. Repeated requests do not repeat cleanup. All paths detach sessions, run the
last-session disconnect hooks, reconcile CONNECTED, finish persistence and drain
connection output for up to `runtime.shutdown_timeout_ms` before aborting remaining
connection tasks. Persistence failures are logged and produce an unsuccessful
process exit.

`@dbck` checks SQLite integrity and repairs supported references, homes, containment
lists and exit sources. It preserves valid list order, resolves competing lists
using the object's claimed location when that list contains it, and appends newly
attached members by dbref. Replacement homes use a safe controlled location, safe
controlled home, configured default home, starting home, then starting room.
Room droptos are stored separately from containment; dropto movement is deferred.
Unreachable rooms without FLOATING, Wizard objects and ordinary occupants inside
Wizard containers are diagnostic findings, not reasons to change flags.

GOD and configured starting/home destinations are protected from GOING purges by
clearing GOING. Other doomed objects become retained Garbage tombstones; surviving
occupants are evacuated and doomed exits are removed. Dbrefs are never reused.
The explicit ownership map in `src/persistence/maintenance.rs` removes owned
accounts/history, Lua state, channel memberships, communication macros, character,
economy and BattleTech records, and clears known dependent references. Unknown
columns on retained rows and unrelated tables remain untouched. An unknown declared
dependency that prevents safe cleanup aborts the transaction.

Repair relocations bypass movement locks and run applicable exit/enter callbacks,
with the moved player's session when available. Callback failures, invalid callback
results and database errors roll back the whole repair and discard pending output.
Destroyed players are notified and disconnected only after commit; their pending
authentication results cannot reattach them. Successful checks return a bounded
summary ending with `Done.`; detailed findings go to server diagnostics.

Startup remains strict and does not repair malformed worlds automatically. This
is an online maintenance command, not an offline recovery tool. It does not invent
missing foundational objects or guess repairs for duplicate accounts or undecodable
records. As with ordinary persistence, only one server may own the database.
