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

`BtechState` and its maps use shared, copy-on-write data so a world checkpoint
can be cloned before an operation. Registration alone does not imply that a
unit has constructed Rust gameplay state. Code that needs a simulated unit
uses the constructed unit or vehicle collections, rather than treating every
registered object as one.

The gameplay modules are organized around focused rules and state transitions:

| Area | Examples in `src/btech/` |
| --- | --- |
| Maps and assets | `map.rs`, `assets.rs`, `state.rs`, terrain and map lifecycle modules |
| Units and equipment | `unit.rs`, `vehicle.rs`, `template.rs`, `loadout.rs`, `equipment.rs` |
| Movement and time | `motion.rs`, `jump.rs`, `power.rs`, `heat.rs`, `simulation_pending.rs` |
| Combat | `shot.rs`, `damage.rs`, `critical.rs`, `artillery.rs`, weapon and ammunition modules |
| Perception | `sensors.rs`, `contacts.rs`, `detection.rs`, LOS and electronics modules |
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
`src/persistence/`.

Map and template assets are decoded by BattleTech asset modules; their game
files remain separate from the SQLite snapshot. Map writes are staged with
other transaction effects and published after a successful world commit.

## Adding BattleTech behavior

Place the rule and its state transition in the focused `src/btech/` module that
owns the invariant. Expose a typed operation through `src/btech/mod.rs` and,
when it is used across the crate boundary, `src/lib.rs`. Adapt that operation
at each needed entry point: a native command, Lua binding, or server tick.

For durable state, update the `BtechState` or owned map/unit model, its
validation, and the corresponding `src/persistence/btech_*.rs` adapter. Keep
user-visible notices in the transaction so a failed save cannot announce an
operation that did not commit. Unit tests live beside rules, and integration
scenarios under `tests/` exercise command, Lua, tick, and persistence behavior.
