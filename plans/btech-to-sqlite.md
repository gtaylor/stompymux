# BTech SQLite persistence TODO

## Scope and safety rules

- [x] Retire `btechdb.finf` and `hcode.db` only after the SQLite-only restart
  gate passes.
- [x] Restart continuation state now resides in the SQLite game database and
  is not BTech persistence.
- [x] Do not store raw structs, pointers, function pointers, cache trees, or
  linked-list nodes. Persist stable scalar values and explicit child rows only.
- [x] Add a BTech persistence schema version and validate it before SQLite
  BTech state is loaded. Version 1 is required.

## Audit the current partial dual-write implementation

- [x] Repair the partial writer before building on it:
  - [x] Ensure every created table has a writer and a reader before it is
    considered implemented.
  - [x] Add prepared-statement/error-path tests for all BTech writers. The
    CTest-only fault matrix forces prepare and execute failure for every
    special-state table and the sparse economy table, preserving the prior
    snapshot on every failure.
  - [x] Add table-level row-count assertions for a fixture containing BTech
    special objects, not only an empty minimal database.
- [x] Map scalar state, base terrain grid, occupancy, LOS matrix, map objects,
  terrain bits, and repair-event tables have initial writers.
- [x] `MECHREP`, `TURRET`, and bounded autopilot scalar tables have initial
  writers.
- [x] `MECH` parent, section, and critical-slot tables have initial writers.
- [x] MECH position, bay/turret links, C3/C3i, timing, and frequency schemas
  now have dual-write writers.
- [x] BTech SQLite readers now restore the represented map, repair, MECH,
  MECHREP, TURRET, and AUTOPILOT state through normal allocations.

## Complete explicit MECH representation

- [x] Write `mech_pd` position state, bay/turret links, and `mech_sd` C3/C3i
  state into explicit tables.
- [x] Write the complete `tic[NUM_TICS][TICLONGS]` matrix and all `FREQS`
  frequency/title/mode slots.
- [x] `btech_mech_runtime`: serialize every persisted `mech_rd` scalar.
  - [x] Exclude the `staggerDamageList` pointer; use its child table below.
  - [x] Timestamp fields are Unix wall-clock timestamps stored as signed
    SQLite `INTEGER` values; scheduling counters remain game ticks.
- [x] `btech_mech_position`: write/read every `mech_pd` scalar.
- [x] `btech_mech_bays`: write/read all bay and turret dbref links.
- [x] `btech_mech_special`: write/read every `mech_sd` scalar.
- [x] `btech_mech_c3_nodes`: write/read C3 and C3i arrays with stable indexes.
- [x] `btech_mech_tics`: write/read the complete `tic[NUM_TICS][TICLONGS]`
  matrix.
- [x] `btech_mech_frequencies`: write/read frequency number, mode, and title
  for all `FREQS` slots.
- [x] `btech_mech_stagger_damage`: write/read damage amount, time, attacker,
  and counted state in linked-list order.
- [x] Reserved `mech_ud` / `mech_rd` fields are retained in indexed child rows
  (`btech_mech_unit_aux` and `btech_mech_runtime_unused`) rather than treated
  as disposable. `unused_char` is also retained.

## Complete explicit AUTOPILOT representation

- [x] Write queued autopilot command enum/position/argument rows without
  persisting the runtime function pointer.
- [x] Write A* path node rows in list order without persisting list pointers.
- [x] `btech_autopilot_commands`: command ordinal, enum, argument count, and
  each argument as a child row. Resolve the function pointer from the enum on
  load; never persist it.
- [x] `btech_autopilot_path`: A* node ordinal and x/y/parent/scores/hex offset.
- [x] Decide and document whether weapon-list and range-profile data are
  deterministic caches.
  - [x] They are deterministic caches derived from the restored MECH critical
    slots and live weapon definitions; `auto_update_profile_event()` rebuilds
    both after parent objects are linked.
  - [x] If deterministic, clear and rebuild them through the normal autogun
    helper after load.
  - [x] The non-deterministic alternative is unnecessary; no cache tables are
    stored.
- [x] Rebuild queued autopilot event state from the durable active
  MECH/AUTOPILOT association and command queue; do not restore raw event
  pointers.

## Complete map and repair representation

- [x] Implement readers for maps, slots, LOS, map objects, terrain bits, and
  the base map grid.
- [x] Use `newfreemap` and normal map-object helpers to allocate/rebuild state;
  do not assign legacy pointer values.
- [x] Recreate map building-regeneration behavior through the normal runtime
  helper after loading `TYPE_BUILD` objects.
- [x] Implement reader for repair events using normal `FIXEVENT`/repair-event
  helpers, preserving fake-event semantics and remaining ticks.
- [x] Validate map dimensions, slot bounds, LOS bounds, object types, and bit
  row sizes before allocating memory.

## SQLite read lifecycle

- [x] Add a post-core-game-load BTech persistence hook: core MUX objects must
  exist before BTech object dbrefs can be validated.
- [x] Initialize BTech registries and the XCODE tree before SQLite BTech load.
- [x] Construct MECH, MECHREP, MAP, AUTOPILOT, and TURRET objects through their
  normal allocation callbacks.
- [x] Restore parent rows before child rows, then run normal BTech consistency,
  map, and cache rebuild helpers.
- [x] Make SQLite BTech loading fail startup on missing required tables,
  invalid schema versions, duplicate keys, invalid dbrefs, or invalid indexes.

## Test plan

- [x] Add a representative BTech fixture containing every special-object type.
  The integration test seeds hardcoded MAP, MECH, MECHREP, AUTOPILOT, and
  TURRET core objects, snapshots them to SQLite, and restarts from SQLite in
  an isolated working directory.
- [x] Cover map objects, terrain bits, occupancy, LOS, repair events, MECH
  sections/criticals/tics/frequencies/C3/stagger history, and autopilot
  commands/path state.
- [x] Verify SQLite dumps preserve every expected row represented by the
  special-object fixture, including map dimensions/grid and MECH fixed
  child-table counts.
- [x] Add SQLite-only restart integration coverage and compare loaded runtime
  state to the seeded fixture's special-object and fixed-child-table coverage.
- [x] Dump again after SQLite-only restart and compare key tables/rows for
  stable round-trip output.
- [x] Corrupt or remove required SQLite rows and assert startup fails before
  accepting live connections.
- [x] Force SQLite snapshot replacement failure and assert the prior complete
  SQLite snapshot, including BTech tables, remains unchanged.
- [x] Exercise normal, crash, killed, and restart dump variants where BTech
  special-object state is expected to persist.
  - [x] Normal, `DUMP_RESTART`, and `DUMP_CRASHED` snapshots are covered by
    the integration fixture and include BTech state.
  - [x] `SIGUSR2` owns `DUMP_KILLED`; its `.KILLED` snapshot is covered by the
    integration fixture.

## Cutover and removal

- [x] Switch BTech reads from `btechdb.finf` / `hcode.db` to SQLite only after
  all prior test gates pass.
- [x] Remove legacy BTech file writers/readers, including `btdb.c`,
  `xcode_io.c`, map/hcode binary serializers, and repair-file serializers.
- [x] Remove the `btfi` CMake target, sources, tests, and include paths once no
  source file depends on Fast Infoset.
- [x] Remove obsolete `hcode_database` configuration after all its data has a
  SQLite replacement.
- [x] Keep MUX restart continuation state separate from BTech persistence,
  while storing it in the SQLite game database.
