---
title: Source layout
linkTitle: Source layout
description: How the Rust codebase is organized
type: docs
weight: 10
---

StompyMUX is a Rust package in `stompymux-rs/`. `src/main.rs` is the process
entry point; `src/lib.rs` exports the server's public types and operations.
The implementation is organized by responsibility across modules.

| Source area | Responsibility |
| --- | --- |
| `src/server/` | Serialized world owner, connection lifecycle, command execution, ticks, commits, and shutdown |
| `src/world/`, `src/accounts/`, `src/state/` | MUX objects, containment and allocation, accounts, and typed object state |
| `src/btech/` | BattleTech models, assets, rules, simulation operations, and special-object commands |
| `src/commands/` | Native command registration, parsing, queues, and command results |
| `src/communication/` | Channels, membership, aliases, speech, and pages |
| `src/lua/` | Lua runtime, sandbox, scripts, flows, and package bindings |
| `src/runtime/` | Transaction checkpoints and staged effects shared by native and Lua operations |
| `src/persistence/` | SQLite loading, validation, and selective writes |
| `src/text/`, `src/telnet/` | Styled and Markdown documents, terminal rendering, Telnet negotiation, and transport |
| `src/help/` | In-game help indexing and rendering (see [Help system](../help-system/)) |
| `src/config/` | Configuration model, directives, and runtime administration |
| `crates/map/` | Battlefield map data shared by the server and map tools: layered hexes, terrain, map flags, hex geometry, the [map file](../map-files/) format, and the `map-check` CLI |
| `crates/template/` | BattleTech unit templates shared by the server and template tools: the [unit template](../unit-templates/) document format, the weapon and system catalogue, loadouts, construction rules and construction mass |
| `crates/mapgen/` | Procedural battlefield map generation library and the `mapgen` CLI; depends on `crates/map` but nothing in the server, so editors can embed it (see [Map generation](../map-generation/)) |
| `crates/mappy/` | The Mappy desktop map editor, built on `crates/map` and iced (`just mappy`) |
| `tests/` | Integration scenarios and fixtures; unit tests also live beside implementations |

The server owns one serialized `World`. Tokio socket tasks send connection
input to it through bounded channels; they do not edit game state directly.
`World` contains ordinary MUX state and `BtechState`, so native commands, Lua
callbacks, and timed BattleTech updates work on the same candidate state.

The server validates and persists a candidate world before publishing its
notifications, file writes, and other effects. `src/runtime/transaction.rs`
keeps those effects and nested savepoints independent of Lua values.
`src/server/transaction.rs` coordinates commit, rollback, and delivery;
`src/persistence/` owns the SQLite representation.

New behavior belongs in the module that owns its invariant. Add a typed domain
operation there, then adapt it at command, Lua, or server entry points as
needed. Add durable fields to the model and persistence adapter together.
Keep protocol conversion in `src/telnet/` or `src/text/`, rather than in domain
rules. The [BattleTech architecture](../btech-architecture/) article describes
that subsystem in more detail.
