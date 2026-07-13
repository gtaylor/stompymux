# BTech SQLite persistence contract

The SQLite game database is the sole durable store for BTech special-object
state. At startup, BTech initializes its normal special-object registry after
the core MUX game database is loaded, then strictly validates and reconstructs
the BTech tables from that same SQLite snapshot. Legacy `btechdb.finf` and
`hcode.db` readers and writers have been removed.

`SIGUSR2` requests a controlled shutdown while preserving a separate
`game_database.KILLED` snapshot. It is intended for operator use when a
last-known-state checkpoint is needed before stopping the server.

## Required object representations

- `MECH`: parent/unit fields; runtime fields; position fields; special-system
  fields; weapon tick matrix; radio-frequency/title data; sections; critical
  slots; bays; turret links; C3 and C3i networks; stagger-damage history.
- `MECHREP`: current target.
- `MAP`: scalar state, occupancy, LOS matrix, map objects, and terrain bits.
- `AUTOPILOT`: scalar state, queued commands, A* path, and any state that is
  not recomputed after restart. Weapon/profile caches must either be rebuilt
  deterministically or stored explicitly.
- `TURRET`: scalar state and timing slots.
- Repair events: mech, event type, remaining ticks, packed event data, and
  fake-event flag.

`time_t` runtime values (weapon recycle, stagger history, and stagger check)
are stored as signed SQLite `INTEGER` Unix wall-clock timestamps. Game update
counters remain game ticks. Autopilot weapon lists and range profiles are not
stored: they are deterministic caches rebuilt with `auto_update_profile_event`
after the MECH and AUTOPILOT parents are linked. MUX autopilot events are also
runtime-only: after a SQLite read, an autopilot still attached to its MECH and
with a non-empty durable command queue receives a fresh `EVENT_AUTOCOM` event.
The normal command dispatcher then rebuilds any goal-specific events it needs.

## Loader requirements

1. Initialize BTech runtime registries after the core MUX game database loads.
2. Construct each special object using its normal allocation callback.
3. Restore scalar rows and child rows in dependency order.
4. Rebuild linked lists, event queues, map indexes, and derived caches using
   normal runtime helpers rather than restoring pointer values.
5. Reject invalid schema versions, duplicate keys, out-of-range indexes, and
   dbrefs absent from the core game database.

## Cutover validation retained by the test suite

1. Seed a representative game with every special-object type, map objects,
   repair events, and autopilot commands/path state.
2. Dump to SQLite, restart with legacy reads disabled, and assert equivalent
   object fields and child-row counts.
3. Make a second dump and compare the SQLite representation for stability.
4. Force malformed SQLite rows and confirm startup fails without changing the
   prior snapshot.
5. The test suite also injects prepare, write, and replacement failures to
   prove a failed dump preserves the last complete SQLite snapshot.

Restart continuation tables are separate from BTech game state, but live in
the same SQLite database so a controlled re-exec can restore MUX sessions.
