---
title: Source layout
linkTitle: Source layout
type: docs
weight: 10
---

The MUX server is organized by responsibility beneath `src/mux`.

- `server` contains platform definitions, configuration parsing, server state,
  lifecycle, logging, timers, signals, and file caches.
- `support` contains reusable containers, buffer helpers, and string utilities.
- `database` owns game objects, attributes, flags, powers, locks, and virtual
  attributes.
- `world` owns player, object, matching, movement, and presentation behavior.
- `commands` owns command dispatch, queues, evaluation, and macros.
- `communication` owns channels, communications attributes, and speech.
- `network` owns client descriptors, Telnet, sockets, and event scheduling.
- `persistence` owns SQLite-backed MUX data.
- `lua` owns the Lua runtime integration.
- `help` owns indexing, rendering, and command handlers for the markdown help
  system (see [Help system](../help-system/)).

Project code includes MUX interfaces through paths rooted at `mux/`; it does
not depend on the include-directory search order. Types exposed by MUX use
descriptive PascalCase names. Functions use snake_case, and implementation
details remain private to their owning module unless another translation unit
needs them.

Mutable resources should be owned by a named subsystem or by the operation
that uses them. For example, each help article index is an independently
created `HelpIndex`, while wildcard captures and lock parser/serializer state
are scoped to a single call. New code should pass these owners or operation
contexts explicitly instead of adding mutable file-scope state. Immutable
lookup tables may remain `static const`.

`MuxServer` is the process composition root. It creates and destroys the
long-lived resources in dependency order:

| Owner | Contains or depends on | Passed to |
| --- | --- | --- |
| `MuxServer` | Configuration, `BtechContext`, and all owners below | Startup and shutdown |
| `GameDatabase` | Object array, cached names, attribute-number index, allocation bounds, freelist, and mark buffer | Database, persistence, world, and command code |
| `PersistenceContext` | Configuration, database, vattrs, channels, macros, snapshot counters, and SQLite extension callbacks | Snapshot loading and writing |
| `MacroRegistry` | Player macro sets and their capacity | Macro commands and commac persistence |
| `ChannelRegistry` | Channel-name index and channel count | Comsys commands, functions, and commac persistence |
| `CommandRegistry` | Built-in commands, prefixes, macros, functions, and user-function ordering | Command dispatch, evaluation, and configuration aliases |
| `WorldIndexes` | Attribute, flag, power, player, forward-list, and parent-command indexes | Database, world, and command modules |
| `AccessControlStore` | Allowed, forbidden, and suspect sites plus disallowed player names | Configuration, connection, and player creation paths |
| `WorldContext` | Borrowed database, configuration, world indexes, and access-control store | Object, matching, lock, and world-facing command operations |
| `ObjectList` | Results for one search or wildcard-attribute operation | Created and destroyed by the calling operation |
| `MuxEventScheduler` | Timed BTech event lists, type index, recycled events, and tick | BTech event producers and lifecycle shutdown |
| `RuntimeClock` | Current time, maintenance deadlines, event cadence, and memory-usage samples | Timers, queues, descriptors, and status commands |
| `MaintenanceContext` | Configuration, clock, descriptors, queues, player cache, background command, and Lua owner slot | `ServerLifecycle` and `ServerTimer` |
| `ServerLifecycle` | libuv loop, service timers, signals, sockets, and one borrowed `MaintenanceContext` | Timers, sockets, and event producers |
| `DescriptorRegistry` | Active client descriptors | Network, Lua, cache delivery, and connection commands |
| `CommandQueue` | Per-object, wait, and semaphore queues | Lifecycle ticks and command producers |
| `CommandContext` | Player, enactor, descriptor, matching state, and one `EvaluationContext` | Interactive and queued command dispatch |
| `CommandInvocation` | Parsed command identity, arguments, vectors, and current `CommandContext` | Uniform typed command handlers and legacy adapters |
| `EvaluationContext` | Function limits, registers, pipes, and trace entries | Evaluator recursion and function handlers |
| `LuaRuntime` | Lua state, loaded modules, schedules, and sandbox package | Lua commands, events, flows, and maintenance ticks |
| `ServerLog` | Logging configuration, recursion depth, timestamp scratch space, and arbitrary-file cache | Process-wide logging entry points |
| `LoginThrottle` | Recent failed-login windows | Connection flow |
| `PlayerCache` | Per-player queue accounting | Queue and player operations |
| `FileCache` | Connect, reject, quit, and rotating connection text | Network and file-list commands |
| `HelpIndex` | Parsed article metadata rooted at one help directory | Help rendering and commands |
| `VattrStore` | Dynamic attribute definitions and their string pool | Database persistence and attribute commands |

Core MUX and BTech source files no longer access the old `mudstate` or
`mudconf` aliases. `MuxServer` owns a narrow `BtechContext` for older BTech
callbacks whose signatures do not yet carry a command context; interactive and
queued dispatch update that context while invoking BTech code. The command
table itself dispatches through typed `CommandInvocation` handlers, so it no
longer stores incompatible function pointers. New and converted APIs take the
narrow owner or operation context they require.
