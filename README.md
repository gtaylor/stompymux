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

## Implementation boundaries

- Tokio handles sockets and timers. A single owner serializes world and Lua
  work, using bounded channels. SQLite and Argon2id run on bounded blocking
  work; Lua values never cross threads. A database commit briefly serializes
  command processing while networking continues.
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
