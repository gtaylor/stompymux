# StompyMUX Rust foundation

An independent Rust implementation of the MUX foundation. The copied game world,
configuration and Lua modules are the compatibility fixtures; no C server code is
linked or invoked. LuaJIT and SQLite are built from vendored dependencies.

## Run the copied world

From this directory, with Rust and a C compiler installed:

```sh
cargo run -- check --game-dir game
cargo run -- import-legacy --source game/data/stompymux.db --game-dir game
cargo run -- serve --game-dir game
```

Import is a one-time operation. If `game/data/stompymux-rs.db` already exists,
skip import and start the server. Import refuses to overwrite it. The server
refuses to silently bootstrap when a populated legacy database exists.

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

`check` reads configuration, validates references, and loads required Lua
modules without writing files. It reads Rust storage when present, otherwise
validates the legacy source. It does not execute startup hooks.

## Storage and import

The supported legacy import format is **schema 32**. The supplied fixture has
16 objects, two accounts, two channels and the already-bootstrapped starting
world. Object IDs, names, account hashes and aliases, login metadata/history,
flags/powers, descriptions, relationships, typed Lua state and channel metadata
are imported. Legacy exit fields are converted into explicit source and
destination relationships; linked-list bookkeeping is not retained.

Rust storage uses SQLite `user_version=1` and a single versioned JSON world
snapshot in the `world` table. Each accepted mutation replaces that snapshot in
one transaction. Failed writes restore the in-memory world and discard pending
success output. This deliberately simple format is suitable for the first
milestone; later versions can migrate to relational tables without changing
the world API. Files are created with mode 0600.

Persistence uses SQLx 0.9 with Tokio and bundled SQLite. Queries run at runtime;
building requires no database, SQLx CLI or query metadata. Each operation opens
and closes its own connection, honoring `database.busy_timeout_ms`. Checks and
legacy reads use read-only connections. The driver change introduces no journal
mode change, connection pool, foreign-key enforcement or schema migration.

The copied legacy database remains unchanged and is the archive for deferred
BattleTech tables, channel membership/history, macros and page-recipient data.
Keep it alongside Rust storage. The importer prints its coverage. There is no
legacy write compatibility or import support for older backup schemas.

For a **new, empty game**, use a separate game-directory copy with no populated
legacy database and no Rust database. `serve` creates foundational objects,
runs the unchanged `bootstrap_world.lua`, and commits the initialized world
once. Random administrator credentials are written to
`bootstrap-credentials.txt` with mode 0600. A stale credentials file blocks
another bootstrap attempt rather than overwriting credentials. Existing/imported
worlds never run first-startup hooks.

## Configuration

All 182 legacy TOML mappings are typed and retained, including options for
features not yet implemented. Twenty additional settings configure the Rust
runtime. See [configuration semantics and runtime defaults](docs/configuration.md)
and the annotated `game/stompymux.toml`.

Listener precedence is CLI → TOML → centralized defaults. Both IPv4 and IPv6
addresses are supported. Content paths resolve relative to `--game-dir` (default
`game`); recursive include paths resolve relative to the including file.

`database.game_database` now names **live Rust storage**, defaulting to
`data/stompymux-rs.db`. `database.legacy_game_database` names the legacy archive,
defaulting to `data/stompymux.db`. Update older configurations to declare both.
No files are moved, migrated or automatically imported. The explicit
`import-legacy --source` argument takes precedence over the archive setting.

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
restore stale connection state. CONNECTED is excluded from saved snapshots and
ignored in imported/default object flags; commands and Lua cannot override it.

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
TCP ports and controlled credentials. They cover import preservation and schema
rejection, bootstrap idempotence, Lua limits/locks, real registration and login,
duplicate registration, password mismatch, throttling, speech, WHO, movement,
multiple sessions, restart durability, injected write failures, stale auth
results, bounded output and graceful shutdown. They do not mutate `game/` or
`../btmux-khi/`.

The original game assets and data were copied intact. Their inherited license
is retained in `LEGACY-LICENSE.md`.
