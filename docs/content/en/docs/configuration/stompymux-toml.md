---
title: stompymux.toml reference
linkTitle: stompymux.toml
weight: 20
---

`game/stompymux.toml` controls the running game server. It is TOML, organized
into sections (`[server]`, `[battletech]`, `[security]`, and so on).
Configuration changes can be made through the appropriate wizard
configuration commands (`@admin`) or by editing the file before starting the
server. `stompymux.toml` includes `game/aliases.toml` (the stock command, flag,
function, and attribute abbreviations) via a top-level `include` array; add
local aliases to `stompymux.toml`'s own `[aliases.*]` tables rather than editing
`aliases.toml` directly, since an including file's keys always win over
anything pulled in through `include`.

## Sections

| Section | Contents |
| --- | --- |
| `[database]` | SQLite game database path, checkpoint dump interval, and mech/map database paths. |
| `[lua]` | Lua module directory plus callback instruction and memory limits. |
| `[server]` | Port, MUD name, and function recursion/invocation limits. |
| `[battletech]` / `[battletech.xp]` | BattleTech gameplay tuning and the XP system. |
| `[mux]` | Base MUX server behavior not covered by a more specific section. |
| `[flags]` | Flags set on new players/exits/rooms/robots/things (`player`, `exit`, `room`, `robot`, `thing` arrays). |
| `[security]` | Password hashing and login rate limiting (see below). |
| `[sites]` | Site ACLs: `forbid`, `suspect`, `trust`, `permit` arrays of `{ address, mask }` tables. |
| `[access.*]` | Per-command/function permission tables (`commands`, `functions`, `lists`, and `config`). |
| `[aliases.*]` | Command, flag, and function alias tables (`commands`, `flags`, and `functions`). |
| `[names]` | `bad`/`good` player-name lists. |
| `[logging]` | `log` and `log_options` bitmask arrays. |

Most directives are plain scalars (`port = 5555`, `fork_dump = true`). A few
directives take other shapes:

- **Flag/bitmask directives** (`[flags]` and `[logging]`) are TOML
  arrays of strings. `logging.log` is negatable: prefix an entry with `!` to
  clear a bit that's on by default (e.g. `log = ["!accounting", "bugs"]`).
- **Alias directives** (`[aliases.*]`) are tables mapping the alias to its
  target, e.g. `"@ch" = "@chown"`.
- **Access directives** (`[access.*]`) are tables
  mapping a command or function name to one or more permissions, e.g.
  `encrypt = "wizard"` or `"@dig" = ["wizard", "need_location"]`.
- Building commands are restricted to Wizards. There is no global building
  toggle or `global_build` command-access permission.
- **Site directives** (`[sites]`) are arrays of `{ address = "...", mask =
  "..." }` tables, applied in file order.

An unrecognized key is logged to stderr and skipped rather than aborting the
whole file; a syntax error in the TOML itself aborts loading.

## Default Lua parents

The `[mux]` section can assign an object-logic module to each newly created
object type:

| Parameter | Shipped value | Applies to |
| --- | --- | --- |
| `default_thing_lua_parent` | `default_thing.lua` | Things |
| `default_room_lua_parent` | `default_room.lua` | Rooms |
| `default_exit_lua_parent` | `default_exit.lua` | Exits |
| `default_player_lua_parent` | `default_player.lua` | Players and robots |

Paths are relative to `game/lua/object_logic`. Empty values disable automatic
assignment for that type. Configuration changes apply only to objects created
afterward and never backfill the database. `@clone` preserves the source
object's Lua parent, including an empty one, instead of using the configured
type default.

## Password and login security

Passwords are stored as Argon2id hashes through the vendored libsodium library.
Each stored hash includes its salt, algorithm, and work factors. Password hashes
are never recoverable, and legacy `crypt(3)` password hashes are not accepted.

| Parameter | Default | Description |
| --- | ---: | --- |
| `player_password_length_limit` | `64` | Maximum password length in characters. Password creation and password changes reject longer values; login attempts longer than this limit are rejected before password hashing. |
| `password_hash_opslimit` | `3` | Argon2id CPU work factor. Increase only after measuring login latency on the game host. Values below `1` disable password hashing and prevent password changes and new player creation. |
| `password_hash_memlimit` | `12582912` | Argon2id memory work factor in bytes (12 MiB). Values below 1 MiB are rejected. A higher value makes offline password guessing harder but consumes more memory per password operation. |
| `login_attempt_burst` | `3` | Number of password operations a source address may make immediately. |
| `login_attempt_refill` | `10` | Seconds required to restore one attempt for a source address. |
| `login_hash_limit` | `5` | Global maximum number of password operations admitted per second. This protects the single game event loop from a distributed login flood. |

These live under `[security]` in `stompymux.toml`.

The per-source tracker has room for 1,024 recent addresses and evicts the least
recently refilled entry when full. A throttled connection receives the same
generic failure response as an invalid login and is disconnected. Keep the
global rate low enough that password verification cannot consume all event-loop
time, and tune the Argon2id settings on the production host rather than aiming
for a one-second hash.

The defaults intentionally favor a responsive telnet game server. They are
lighter than libsodium's interactive preset, so the firewall or host should
also rate-limit new TCP connections to the game port.
