---
title: stompymux.toml reference
linkTitle: stompymux.toml
description: Run-time configuration
weight: 20
---

`game/stompymux.toml` controls the running game server. It is TOML, organized
into sections (`[server]`, `[battletech]`, `[security]`, and so on).
Configuration changes can be made through the appropriate wizard
configuration commands (`@admin`) or by editing the file before starting the
server. `stompymux.toml` includes `game/aliases.toml` (the stock command and
flag abbreviations) via a top-level `include` array; add
local aliases to `stompymux.toml`'s own `[aliases.*]` tables rather than editing
`aliases.toml` directly, since an including file's keys always win over
anything pulled in through `include`.

## Sections

| Section | Contents |
| --- | --- |
| `[database]` | SQLite game database path, first-run bootstrap objects, checkpoint behavior, and mech/map database paths. |
| `[lua]` | Lua module directory, VM memory limit, and persistent object-state limits. LuaJIT compilation is enabled. |
| `[server]` | Port and MUD name. |
| `[colors]` | Case-insensitive named RGB colors used by styled-text markup. |
| `[osc8.presets]` | Case-sensitive session-scoped Mudlet OSC 8 presets. |
| `[battletech]` / `[battletech.xp]` | BattleTech gameplay tuning and the XP system. |
| `[mux]` | Base MUX behavior, including default flags and Lua parents for newly created objects. |
| `[security]` | Password hashing and login rate limiting (see below). |
| `[sites]` | Site ACLs: `forbid`, `suspect`, `trust`, `permit` arrays of `{ address, mask }` tables. |
| `[access.*]` | Permission tables for commands, lists, and configuration directives. |
| `[aliases.*]` | Command and flag alias tables. |
| `[names]` | `bad`/`good` player-name lists. |
| `[logging]` | The `log_options` formatting array and a `[logging.topics]` table of event-category booleans. |

Lua callbacks run with LuaJIT compilation enabled. Consequently, `[lua]` has no
per-callback instruction limit: use bounded loops in trusted Lua modules.
`state_value_limit` caps one persistent string, `state_entry_limit` caps the
number of state entries on an object, and `state_object_limit` caps the total
bytes of keys and values on an object. Their defaults are 65536, 1024, and
1048576 respectively.

Most directives are plain scalars (`port = 5555`, or, under `[database]`,
`fork_dump = true`). A few
directives take other shapes:

- **Flag/bitmask directives** (`mux.default_player_flags`,
  `mux.default_exit_flags`, `mux.default_room_flags`,
  `mux.default_thing_flags`, and `logging.log_options`) are TOML arrays of
  strings. Logging event categories instead use individual booleans such as
  `[logging.topics] security = true` and `all_commands = false`.
- **Alias directives** (`[aliases.*]`) are tables mapping the alias to its
  target, e.g. `"@cr" = "@create"`.
- **Access directives** (`[access.*]`) are tables
  mapping a command or list name to one or more permissions, e.g.
  `"@dig" = ["wizard", "need_location"]`.
- Building commands are restricted to Wizards. There is no global building
  toggle or `global_build` command-access permission.
- **Site directives** (`[sites]`) are arrays of `{ address = "...", mask =
  "..." }` tables, applied in file order.
- **Named colors** (`[colors]`) map a color name to an array of three integer
  RGB channels. Each channel must be from `0` through `255`.
- **OSC 8 presets** (`[osc8.presets]`) map a preset name to a string of the
  flattened directives accepted on a styled link.
- **Bootstrap objects** (`[database.bootstrap.objects]`) map numeric dbrefs to
  inline tables containing a `type` (`room` or `player`) and `name`.

An unrecognized key is logged to stderr and skipped rather than aborting the
whole file; a syntax error in the TOML itself aborts loading.

## BattleTech repair timing

`battletech.techtime_multiplier` scales the duration of newly scheduled repair
work. It accepts finite values from `0.0` through `10.0`: `1.0` is the baseline,
`0.5` halves repair time, and `1.5` uses 150% of baseline. A value of `0.0`
adds no player tech-time debt and completes each newly scheduled repair on the
event scheduler's one-second minimum delay, regardless of older debt.

Wizards may change the live value with
`@admin btech_techtime_multiplier=<value>`. Existing tech-time debt and event
deadlines are not rescaled. New work uses the new value; follow-up units in an
armor or internal-repair batch use the value current when each unit is
scheduled.

## New database bootstrap

When `database.game_database` does not exist, startup creates the configured
bootstrap objects, writes a complete SQLite snapshot atomically, and then
opens the listener. Existing files are always loaded and are never replaced by
bootstrap, including empty or corrupt files.

```toml
[database.bootstrap.objects]
0 = { type = "room", name = "Limbo" }
1 = { type = "player", name = "GOD", wizard = true }
2 = { type = "player", name = "Wizard", wizard = true }
3 = { type = "room", name = "Used Mech Store" }
4 = { type = "room", name = "Starter Room" }
5 = { type = "room", name = "Afterlife" }
```

The stock configuration uses `#3` for `usedmechstore`, `#4` for both player
starting directives, and `#5` for `afterlife_dbref`. Bootstrap fails before
writing if the required dbrefs are missing or have incompatible types. Seeded
objects receive the configured default flags and Lua parents for their types.
Player entries accept an optional `wizard` Boolean that defaults to `false` and
cannot be enabled for rooms. The stock `#1` and `#2` entries enable it and
receive distinct generated passwords, printed to stderr once after the database
is safely published. When this table is present, it replaces the compiled
bootstrap object defaults rather than extending them. Startup fails if `#1` is
not a player with `wizard = true`, even when the database already exists. The
startup log records when bootstrap begins and lists each created object's name,
dbref, and type, regardless of the `logging.topics.startup` setting.

The compiled fallbacks use bootstrap-safe room references: `#0` for the player
starting room and default home, `#3` for the used mech store, and `#5` for the
afterlife. The stock TOML intentionally overrides the player starting room to
`#4` Starter Room. It does not set `default_home`, so that value retains the
compiled `#0` fallback.

## Named colors

The `[colors]` table defines custom names accepted by `[fg=NAME]`,
`[color=NAME]`, and `[bg=NAME]` styled-text markup:

```toml
[colors]
"brand-blue" = [32, 96, 192]
```

Names are case-insensitive and may contain letters, digits, hyphens, and
underscores. A name may be at most 60 characters. Custom names must not overlap
an opaque CSS/X11 built-in name; a case-insensitive collision is a fatal
configuration error and stops server startup. Built-ins are immutable and do
not need entries in this table.

RGB colors are emitted directly for truecolor clients and mapped to the nearest
xterm-256 or ANSI-16 color for less capable clients. A built-in name collision
stops startup; other malformed entries are logged and skipped. When
configuration files are included, normal table merge rules apply, so a color
in the including file overrides a color with the same TOML key in an included
file.

## OSC 8 presets

The `[osc8.presets]` table defines reusable Tier 2–5 link configuration:

```toml
[osc8.presets]
button = 'color=white bg=green bold hover.bg=darkgreen'
poll = 'selection.group="demo-poll" selection.exclusive'
```

Names are case-sensitive, contain 1–60 URI-safe letters, digits, dots,
underscores, tildes, or hyphens, and must begin with a letter or digit. Values
use the same flattened directives accepted inside a styled link tag. Presets
may be partial templates, but every link's merged result must be valid. Invalid
preset configuration stops startup. Definitions are immutable while the
server runs and are sent once to each capable connection.

## Default Lua parents

The `[mux]` section can assign an object-logic module to each newly created
object type:

| Parameter | Shipped value | Applies to |
| --- | --- | --- |
| `default_thing_lua_parent` | `default_thing.lua` | Things |
| `default_room_lua_parent` | `default_room.lua` | Rooms |
| `default_exit_lua_parent` | `default_exit.lua` | Exits |
| `default_player_lua_parent` | `default_player.lua` | Players |

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
| `player_password_length_limit` | `64` | Maximum password length in UTF-8 bytes. Password creation and password changes reject longer values; login attempts longer than this limit are rejected before password hashing. |
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
