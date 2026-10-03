---
title: BattleTech Architecture
weight: 25
description: How BattleTech state, rules, commands, Lua, and persistence fit into StompyMUX
---

BattleTech is implemented in the `stompymux-rs` Rust crate. Its gameplay code
lives in `src/btech/`, while the server, Lua bindings, and SQLite adapter live
in their respective crate modules. `src/lib.rs` exports the BattleTech types
and operations used across those boundaries.

## State and ownership

`World` owns a `BtechState` alongside ordinary MUX objects and accounts. The
BattleTech state holds special-object registrations, saved maps and unit
identities, constructed units and vehicles, and related runtime state such as
turn timing, inventories, character data, and recovery. MUX objects and
BattleTech records refer to the same object IDs.

Registration alone does not imply that a unit has constructed Rust gameplay
state. Code that needs a simulated unit uses the constructed unit or vehicle
collections, rather than treating every registered object as one.

## Transactions and rollback

The world's object, account and channel tables, and the per-entity collections
in `BtechState` (units, vehicles, maps, autopilots, characters and so on), are
`SharedMap`s. Cloning the world only bumps reference counts, and the first
write after a clone copies just the entry being changed. Rollback is therefore
cheap, and two helpers cover it:

- `Scripts::atomic(|before| ...)` runs a native action. If it fails, the world
  and every effect it staged (messages, logs, map writes) return to their state
  before the action. `before` is that earlier world, for comparisons.
- `World::attempt(|world| ...)` does the same for an operation that only
  changes the world.

A shot resolves in a private copy of the world (`ShotCandidate`) and commits it
only on success, so effects inside the shot mutate that copy directly.

The server validates the whole world once per committed transaction and rolls
the transaction back if validation fails. `World::validate_action` and
`BtechState::validate_action` re-check after individual gameplay operations in
debug and test builds only, so a broken operation fails where it happened.
Commands that rely on validation to reject wizard or player input call
`validate` directly.

The server also keeps the world as it last saved it. Each commit writes only
the rows that differ from that baseline, skipping every entry still shared
with it, and a transaction that changed nothing never touches the database.
After a failed save, a maintenance repair, or any write outside the ordinary
commit path, the baseline is dropped and the next save reads the stored world
instead. The baseline also carries the containment-list slots as stored, so a
save never reads the object table back to order contents and exits.

The server writes through one connection kept open for its whole run
(`persistence::Database`): the schema is validated and write-ahead logging
enabled once, prepared statements stay cached, and SQLite folds the log back
into the database at its normal checkpoint interval rather than after every
save. The connection is dropped after a failed write, so the next save opens a
fresh one. Reads use short-lived read-only connections.

The gameplay modules are organized around focused rules and state transitions:

| Area | Examples in `src/btech/` |
| --- | --- |
| Maps and assets | `map.rs`, `assets.rs`, `state.rs`, terrain and map lifecycle modules |
| Units and equipment | `unit.rs`, `vehicle.rs`, `template.rs`, `template_document.rs`, `loadout.rs`, `equipment.rs` |
| Movement and time | `motion.rs`, `jump.rs`, `power.rs`, `heat.rs`, `simulation_pending.rs` |
| Combat | `shot.rs`, `damage.rs`, `critical.rs`, `artillery.rs`, weapon and ammunition modules |
| Perception | `perception/` (sensor band, sight, probes, radar, acquisition), `contacts.rs`, `scanner.rs`, LOS and electronics modules |
| Commands and output | `special_dispatch.rs`, `special_commands.rs`, command and report modules |

These are source areas, not independent subsystems with separate world owners.
Rules may share typed state and operations within `btech`; command handlers
adapt player input and publish the resulting notices.

## Commands, Lua, and the simulation tick

The command system admits BattleTech special-object commands through
`special_dispatch.rs`. `special_commands.json` supplies ordered command and
help metadata; native Rust handlers perform the operations. A selected MUX
object and its BattleTech registration determine which special commands are
available.

The `btech` Lua package is assembled in `src/lua/packages/btech/`. Its binding
modules convert Lua arguments, call Rust operations against the shared world,
and return detached Lua values. `facade.rs` installs the public package and
subpackages. Lua is an adapter to the native state and rules, not a second
BattleTech state store.

`src/server/btech.rs` coordinates the one-second BattleTech simulation step.
It advances pending movement, combat, environmental effects, and timers by
calling domain operations. The server also coordinates normal command
execution, persistence, and output delivery; socket tasks do not mutate the
world directly.

## Commit boundary and persistence

BattleTech changes participate in the same serialized world transaction as
other MUX changes. An operation starts from a world checkpoint and stages
notifications and other effects. The server validates and saves the candidate
world before publishing those effects. If a BattleTech tick or save fails, the
candidate world and its staged effects are rolled back.

`src/persistence/` owns the SQLite representation. `btech.rs` loads and
coordinates BattleTech records, while focused `btech_*.rs` adapters handle
maps, units, vehicles, characters, and other saved features. The persistence
layer validates supported changes and writes them in a database transaction.
Its tables are a storage format, not a direct serialization of Rust struct
layouts. Fresh databases are initialized from the SQL schema files in
`src/persistence/`: `schema32.sql` for the reference tables and
`btech_schema.sql` for every Rust-owned table. Opening a database checks that
all of them exist; there is no lazy table creation and no upgrade of databases
from older builds.

Saved BattleTech state uses typed columns with `CHECK` constraints, and newer
tables are `STRICT`. Collections such as artillery queues, map-object
traversal order and autopilot orders and feedback
are stored one row per entry, so a save touches only the entries that changed.
Enums are stored as integer codes documented beside each table. Dice streams
are stored as their generator key, stream, block and word columns.

Countdowns in those tables are stored as deadlines on the simulation clock
(`btech_simulation_clock`) rather than as seconds remaining: artillery
arrivals, consciousness recovery, building repair, fire
spread and burnout, smoke expiry and the reactor startup window.
A timer counting down in step with the clock keeps the same deadline, so its row
is written only when it starts, is rescheduled or finishes. Loading subtracts
the saved clock to rebuild each countdown. The clock stops while the server is
down, so timers resume rather than expiring during downtime. The shared
conversion lives in `src/persistence/btech_deadlines.rs`.

The clock row itself, which also holds the turn phase as a fixed offset from
the clock, is written alongside any other change. A heartbeat that changes
nothing else writes nothing until `database.clock_save_interval` seconds have
passed; a crash then restores the world exactly as of the last stored second.
Explicit saves and shutdown always store the current clock.

Units and vehicles are the exception to typed columns: each row holds the
record as JSON in two parts. `unit` holds the core (construction, damage,
settings and contacts), which changes rarely, and `live` holds frequently
changing state such as motion, heat and dice, with fields at their default
value left out. The `saved_parts!` lists in `unit.rs` and `vehicle.rs` decide
which part each field belongs to. The values that count once per second in
either part, such as weapon recycle, stun, startup, hiding and the overheat and
stagger clocks, are declared in `src/btech/timers.rs`, `unit_timers.rs` and
`vehicle_timers.rs` together with how each moves at the moment of saving:
counting down, counting up, wrapping around a thirty-second cycle, or held
still, for example weapon recycle on a shut-down unit. They read as zero in the
JSON and are stored one row each in `btech_unit_timers` and
`btech_vehicle_timers`, as the held value or the simulation second at which the
counter reaches or was zero, so a running counter's row does not change from
tick to tick. A save compares each part's stored text and each timer row with
the baseline world and writes only what differs, so a running unit that is
standing still is not written each tick; its remaining writes are real changes
such as the dice rolled by turn-boundary checks. Moving units still write their
motion every tick.

Map and template assets are decoded by BattleTech asset modules (unit templates
are TOML documents; see [Unit templates](../unit-templates/)); their game
files remain separate from the SQLite snapshot. Map writes are staged with
other transaction effects and published after a successful world commit.

## Adding BattleTech behavior

Place the rule and its state transition in the focused `src/btech/` module that
owns the invariant. Expose a typed operation through `src/btech/mod.rs` and,
when it is used across the crate boundary, `src/lib.rs`. Adapt that operation
at each needed entry point: a native command, Lua binding, or server tick.

For durable state, update the `BtechState` or owned map/unit model, its
validation, and the corresponding `src/persistence/btech_*.rs` adapter. A new
`BattleUnit` or `BattleVehicle` field must also be listed in its `saved_parts!`
classification, which fails to compile until it is: `core` for data that
changes rarely, `live` for per-tick state with a serde default, or
`live_always` for per-tick state that is always written. Keep
user-visible notices in the transaction so a failed save cannot announce an
operation that did not commit. Unit tests live beside rules, and integration
scenarios under `tests/` exercise command, Lua, tick, and persistence behavior.
