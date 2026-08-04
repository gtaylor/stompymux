---
title: "BattleTech Architecture"
weight: 25
---

# BattleTech architecture

BattleTech is a domain-oriented package rooted at `src/btech`. Its public
boundary is `src/btech/include/btech`; MUX code must not include any other
BattleTech directory.

## Ownership

Each domain owns both its state and the operations that change that state:

| Domain | Primary ownership |
| --- | --- |
| `core` | Runtime context, random generator, events, and heartbeat |
| `special` | Native special-object registry and typed object operations |
| `map` | Battle maps, terrain, map objects, and cached LOS state |
| `unit` | Mechs, templates, parts, sections, critical slots, and weapons |
| `movement` | Ground, jump, aerospace, DropShip, and landing behavior |
| `sensors` | Contacts, LOS, scanners, ECM, C3, TAG, and spotting |
| `combat` | Attacks, damage, criticals, missiles, artillery, and ejection |
| `repair` | Repair facilities, jobs, validation, and repair events |
| `autopilot` | Autopilot state, commands, paths, targeting, and autogun |
| `economy` | Stores, cargo, and part costs |
| `character` | Skills, experience, advantages, and personal combat |
| `ui` | Shared menu, notification, and presentation primitives |
| `scripting` | Native XCODE-value and script-function adapters |
| `persistence` | SQLite schema and domain persistence adapters |
| `integration` | Narrow adapters to MUX-owned services |

Concrete `Mech`, `BattleMap`, `Autopilot`, and runtime-context layouts are
private. Cross-domain interfaces use forward declarations, database object
references, or domain operations rather than copying another domain's state.

## Public boundary

`BtechContext` is opaque outside the package. `MuxServer` constructs it with a
`BtechDependencies` value whose members are borrowed, and destroys it before
the event scheduler and borrowed services. Public headers separately cover
context ownership, command dispatch, lifecycle producers, special objects, and
persistence.

Adding a public operation requires an owning domain implementation and a narrow
header under `include/btech`. Do not expose a concrete domain structure merely
to avoid writing an operation.

## Persistence

The BTech SQLite schema has its own version and is intentionally independent of
in-memory structure layouts. Schema version 2 has no version-1 migration. An
offline reset operation removes the BTech extension tables whose names begin
with `btech_`. It preserves `btech_object_state`, which belongs to the core MUX
snapshot schema. The server never performs this destructive reset
automatically.

Shut down `stompymux` before resetting or replacing the game database. A normal
dump creates the current BTech tables again.

## Adding code

Place new state and behavior in the domain that owns the invariant. Commands,
events, persistence records, and presentation adapters should call that domain
through typed interfaces. Headers include their direct dependencies and must
compile without relying on inclusion order. Constants use uppercase names,
types use PascalCase, and functions use subject-prefixed snake_case.
