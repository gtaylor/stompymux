---
title: BattleTech Architecture
weight: 25
description: An architectural overview of the codebase
---

BattleTech is a domain-oriented package rooted at `src/btech`. Its public
boundary is `src/btech/include/btech`; MUX code must not include any other
BattleTech directory.

## Ownership

Each domain owns both its state and the operations that change that state:

| Domain | Primary ownership |
| --- | --- |
| `core` | Runtime context, xoshiro256** random generator, events, and heartbeat |
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

The context-owned gameplay generator is xoshiro256**, seeded once from Linux
OS entropy during BTech startup. Its runtime state is not persisted.

Internal command boundaries use the shared MUX `parse_*_checked` helpers for
numeric input. They reject empty values, overflow, non-finite floating-point
values, and trailing non-whitespace input. Presentation code that incrementally
builds text uses `BtechTextBuilder`, which always terminates a non-empty
destination and records truncation instead of writing past its capacity.

Map files are read and written as plain text. Compressed map files are not
supported, and BTech file handling never invokes a shell command.

Autopilot runtime events are adapters around deterministic policy operations.
Path transitions and route construction, weapon eligibility and heat budgets,
physical-side selection, sensor selection, and queued-order ownership can be
tested without a live event scheduler. The adapters gather `Mech` and
`BattleMap` state, apply the policy result through the normal domain APIs, and
retain responsibility for notifications and event scheduling. Queued orders
are bounded, owning values; unsupported definitions and malformed argument
lists are rejected before the queue changes.

Concrete `Mech`, `BattleMap`, `Autopilot`, and runtime-context layouts are
private. Cross-domain interfaces use forward declarations, database object
references, or domain operations rather than copying another domain's state.
`unit/mech_internal.h` may only be included by unit sources. Runtime Mech
access goes through the typed domain APIs; the former `unit/mech_macros.h`
umbrella is intentionally absent. Caller-controlled returns and notifications
are ordinary C control flow; `core/legacy_macros.h` is also intentionally
absent. The only function-like macros retained in BTech are the five command
invoker declaration/definition generators.

Special-object layouts and lifecycle definitions in
`special/registry_internal.h` remain inside `special`. Commands use the narrow
`special/command_registry.h` invocation contract; immutable command catalogues
live beside their owning domains.

The dependency direction is deliberately shallow:

```text
MUX -> include/btech -> integration/commands
                         |
            ui/scripting/persistence
                         |
 gameplay domains -> unit/map/special -> core
```

Presentation, persistence, and MUX adapters call domain operations. They do not
receive mutable layout views. Template discovery, named-field template parsing,
and template cache ownership are unit responsibilities; repair only supplies
the player commands that invoke those operations.

## Public boundary

`BtechContext` is opaque outside the package. `MuxServer` constructs it with a
`BtechDependencies` value whose members are borrowed, and destroys it before
the event scheduler and borrowed services. Public headers separately cover
context ownership, command dispatch, lifecycle producers, special objects, and
persistence.

The concrete layout lives in `core/context_internal.h`; implementation files
include it explicitly when needed, and headers never include it.

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

- Add a domain operation to the smallest focused API owned by that domain, then
  call it from the command or adapter. Do not add a generic field accessor.
- Add a command descriptor beside its owning domain behavior, preserving the
  command spelling, authorization flags, help, and argument handling.
- Add an event with a typed scheduling operation and keep cancellation in the
  lifecycle of the object that owns the event.
- Add a persisted field to the domain snapshot first, then teach the SQLite
  adapter to store and restore that snapshot. Persistence never reads a private
  layout.
- Add weapons and immutable catalogue entries in `unit`; gameplay domains query
  the catalogue through unit interfaces.
- Add a special-object type with its lifecycle and command descriptors in
  `special`, exposing only typed lookup and dispatch operations to callers.

The architecture check rejects files over 800 lines, dotted or generated-style
filenames and prototype banners, private unit or registry headers outside their
owner, complete `Mech` values outside `unit`, mutable or centralized command
catalogues, untyped command callbacks, disabled legacy code, shell-based BTech
file handling, weak numeric parsing at converted command boundaries, and known
legacy exported names.
