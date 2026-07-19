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
| `PersistenceContext` | Borrowed configuration, database, vattrs, channels, macros, snapshot counters, and an owned bounded SQLite extension registry | Snapshot loading and writing |
| `MacroRegistry` | Player macro sets and their capacity | Macro commands and commac persistence |
| `ChannelRegistry` | Channel-name index and channel count | Comsys commands, functions, and commac persistence |
| `CommandRegistry` | Built-in commands, prefixes, macros, functions, and user-function ordering | Command dispatch, evaluation, and configuration aliases |
| `WorldIndexes` | Attribute, flag, power, player, forward-list, and parent-command indexes | Database, world, and command modules |
| `AccessControlStore` | Allowed, forbidden, and suspect sites plus disallowed player names | Configuration, connection, and player creation paths |
| `WorldContext` | Borrowed database, configuration, world indexes, access-control store, and descriptor registry | Object, matching, lock, and world-facing command operations |
| `ObjectList` | Results for one search or wildcard-attribute operation | Created and destroyed by the calling operation |
| `MapText` | One rendered tactical-map buffer and its line view | Created and destroyed by the map or navigation command |
| `MuxEventScheduler` | Timed BTech event lists, type index, recycled events, and tick | BTech event producers and lifecycle shutdown |
| `RuntimeClock` | Current time, maintenance deadlines, event cadence, and memory-usage samples | Timers, queues, descriptors, and status commands |
| `ConfigurationContext` | Borrowed configuration, database, log, command and world registries, world context, and administrative notification context | Startup configuration parsing and runtime configuration commands |
| `ServerControl` | Borrowed configuration, database, log, descriptors, player cache, persistence, lifecycle, and diagnostic command context | Database dumps, shutdown, and signal handling |
| `ConnectionRuntime` | Borrowed configuration, clock, descriptor registry, log, access control, and file-cache owner slot | Telnet listeners and accepted connections |
| `MaintenanceContext` | Borrowed server control, connection runtime, configuration, clock, descriptors, queues, player cache, background command, Lua services, and Lua owner slot | `ServerLifecycle` and `ServerTimer` |
| `ServerLifecycle` | libuv loop, service timers, signals, sockets, and one borrowed `MaintenanceContext` | Timers, sockets, and event producers |
| `DescriptorRegistry` | Active client descriptors | Network, Lua, cache delivery, and connection commands |
| `CommandQueue` | Per-object, wait, and semaphore queues plus a narrow borrowed `CommandQueueDependencies` service view | Lifecycle ticks and command producers |
| `CommandContext` | Player, enactor, descriptor, matching state, borrowed world/log/BTech services, and one `EvaluationContext` | Interactive and queued command dispatch |
| `CommandRuntime` | Borrowed command-facing services, configuration and server-control capabilities, reloadable Lua owner slot, and process status values | `CommandContext` and `EvaluationContext` without exposing `MuxServer` |
| `CommandInvocation` | Parsed command identity, arguments, vectors, and current `CommandContext` | Uniform typed command handlers and legacy adapters |
| `EvaluationContext` | Function limits, registers, pipes, trace entries, and borrowed world/log/BTech services | Evaluator recursion and function handlers |
| `LuaServices` | Borrowed configuration, database, descriptors, command queue, clock, background notification context, log, and process counters | `LuaRuntime` without exposing `CommandRuntime` or `MuxServer` |
| `LuaRuntime` | Lua state, loaded modules, schedules, sandbox package, and one borrowed `LuaServices` view | Lua commands, events, flows, and maintenance ticks |
| `ServerLog` | Logging configuration, recursion depth, timestamp scratch space, and arbitrary-file cache | Process-wide logging entry points |
| `LoginThrottle` | Recent failed-login windows | Connection flow |
| `PlayerCache` | Per-player queue accounting | Queue and player operations |
| `FileCache` | Connect, reject, quit, and rotating connection text | Network and file-list commands |
| `HelpIndex` | Parsed article metadata rooted at one help directory | Help rendering and commands |
| `VattrStore` | Dynamic attribute definitions and their string pool | Database persistence and attribute commands |

Core MUX and BTech source files no longer access the old `mudstate` or
`mudconf` aliases. Context headers mark non-owning members as borrowed;
their destroy functions must never release those dependencies. Compact
concrete owner types with public layouts are embedded in `MuxServer`; opaque
owners and the large heap-resident `ServerConfiguration` are held through
pointers and use paired `*_create`/`*_destroy` functions. `LuaOwner` provides a
stable, explicitly typed owner slot: reload replaces its runtime while queued
flows retain a stable reference to the wrapper.

Only composition and startup functions accept `MuxServer *`. Runtime modules
receive the narrow borrowed context they need: configuration code cannot reach
network ownership, socket acceptance cannot reach persistence, and command
control paths cannot recover the composition root. Source files include these
context interfaces directly instead of using `mux_server.h` as an aggregate
dependency header.

`MuxServer` owns a narrow `BtechContext` for BTech runtime services and legacy
callback objects. In addition to borrowing core MUX services, this context owns
the BTech special-object registry, special command indexes, map-coding cache,
part-name registry, lazily populated template filename registry, combat
overrides, advanced-economy part costs, ANSI colorization indexes, heartbeat
timer state, random-generator state and roll statistics, startup-resolved
missile cluster indexes, wizard-adjustable weapon recycle/BV settings, and
update timestamps. Those resources are released explicitly before the event
scheduler during server teardown. Canonical weapon definitions remain
immutable; runtime overrides are isolated in the context-owned settings array.
Interactive and queued dispatch push a checked `BtechCommandScope` while
invoking BTech code; nested command execution restores the previous scope in
LIFO order instead of manually swapping one mutable command pointer.
Persistence extensions retain their subsystem-specific borrowed context
explicitly, so BTech snapshot code does not use the unthreaded accessor. The
command table itself dispatches through typed `CommandInvocation` handlers, so
it no longer stores incompatible function pointers. BTech notification, lock,
menu, and guard helpers now take their `BtechContext` or `EvaluationContext`
explicitly; the process-global unthreaded accessor and ambient evaluation macro
have been removed. Linting rejects either compatibility seam if it returns. New
and converted APIs take the narrow owner or operation context they require.
Callback and traversal APIs likewise carry an explicit operation context, so
path scoring, radio relay searches, status menus, neighboring-hex visits,
debug-memory walks, and persistence event visits keep their scratch state on
the initiating call rather than at file scope. BTech status, armor, weapon,
target, and scan-report renderers receive their `EvaluationContext` explicitly;
only legacy command and notification entry points resolve the checked current
command scope.

Long-lived mutable process ownership is now concentrated at the POSIX
signal-handler dispatch bridge. Compiled BTech formatting paths no longer use
function-local static result buffers: formatters either return small values or
write into storage owned by the enclosing operation. This includes xcode value
callbacks, status renderers, menu callbacks, generated part names, attribute
reads, mech identifiers, and unit-parts summaries. Random-generator state and
its roll statistics, startup-resolved missile cluster indexes, map coding, ANSI
colorization, and generated part-name indexes are explicit `BtechContext`
owners, while artillery and map-object recycler lists have been replaced by
direct scheduler or allocation ownership. Player map colors are caller-owned
render state, and the LOS tracer writes into an operation-owned `LosTrace`
instead of sharing a process-static coordinate array. Template filename and
reference-mech caching have explicit `BtechContext` lifetimes. Tactical map
rendering returns an owned `MapText` instead of reusing static sketch and
colorization buffers. Advanced-economy prices are also owned by that context;
persistence receives only a short-lived read view of their canonical part
ranges. Repair-job tables and map-link update counters are caller-owned command
scratch. Turret, weapon-recycle, and physical-XP overrides are grouped under
`BtechContext` instead of shared by the process. Keeping that distinction
explicit prevents unavoidable boundary state from becoming a justification for
new ambient state.
