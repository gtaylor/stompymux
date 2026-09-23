# Ground autopilot gameplay audit

This audit records the gameplay checks that are intentionally kept separate from
the unit-level queue and navigation tests.  Every scenario uses an isolated
temporary database and a disposable battlefield; no live game database is
opened.

## Covered behavior

The audit fixture checks that an attached controller can be observed while its
unit is powered off.  It reports the unit's own readiness, returns no current
contacts until the unit is running and placed, and filters remembered contacts
by map and the 30-second sighting window.  A future-dated sighting is ignored so
clock skew cannot expose stale tactical state.

Routing and deterministic yielding revalidate acquired contacts against live
visibility. A regression keeps stale acquisition flags while hiding, moving,
and destroying an enemy; none of those hidden changes can enter the occupancy
snapshot. Enemy destruction affects occupancy only when observable.

Attack movement and pursuit consume that same filtered memory rather than the
controller's raw durable map.  A current friendly contact invalidates an old
hostile assignment, so stale or cross-map data cannot direct a unit.

A one-hex-wide wall barrier produces an `unreachable` failure.  The controller
stops before becoming blocked, retains the failed order, and does not treat a
bounded search as a successful route.  This covers the runtime's route failure
and stop transaction at the smallest reproducible map size.

The surface fixture instantiates tracked, wheeled, and hover vehicles without
pilots on a map containing both water and bridge tiles.  It submits ordinary
move orders through the Lua facade and verifies that the units are admitted as
ground controllers rather than rejected as unsupported chassis.  The dedicated
traversal unit tests cover positive costs and explicit water, ice, and bridge
risk classification; the surface scenario verifies those rules reach all three
vehicle classes.

Two uncrewed units exercise competing route service with deterministic
occupancy handling. Their arrival regions remain distinct so a temporary yield
can recover after the other unit advances. The audit also checks successful and
rejected manual speed controls, weapons hold, a zero projected-heat ceiling,
a multi-mount heat-ceiling regression, and a production heartbeat commit rejected by an injected SQLite trigger. The
latter asserts that the candidate world, movement intent, and feedback remain
unchanged after rollback; transient planning is discarded by the production
harness.

The admission unit tests in `src/btech/power.rs` cover both a BattleMech and a
ground vehicle: an assigned unconscious pilot is rejected by autopilot control,
while an uncrewed chassis remains admitted through the normal no-pilot path.
The same gates are used by autonomous movement and ordinary direct fire.

The existing autopilot runtime and order suites cover the complementary
long-running behaviors: follow movement, patrol cycling, attack-move pursuit
and resumption, successful versus rejected manual controls, restart intent
round-trips, explicit sensor-filtered attacks, and the ordinary uncrewed Mech
path.  Their fixtures also assert that hidden contact data is absent from Lua
observations and that an explicit weapons-heat ceiling prevents an inadmissible
shot.

## Deliberate gaps

The audit does not claim a statistically meaningful 100-controller benchmark;
that is the responsibility of the benchmark fixture and its documented p95
measurement. It also does not make a random combat-damage assertion: dice
outcomes can miss while the shot admission path remains correct.

The wider behavior matrix is covered by existing isolated fixtures:

| Behavior | Regression surface |
| --- | --- |
| Queue IDs, replacement, cancellation, retry | [`tests/btech_autopilot_orders.rs`](../../tests/btech_autopilot_orders.rs) |
| Follow, patrol, attack-move, lifecycle and restart | [`tests/btech_autopilot_runtime.rs`](../../tests/btech_autopilot_runtime.rs) |
| A* cost, invalidation, expansion and record budgets | [`src/btech/autopilot/navigation.rs`](../../src/btech/autopilot/navigation.rs), [`src/btech/autopilot/scheduler_tests.rs`](../../src/btech/autopilot/scheduler_tests.rs) |
| Assigned crew health and recovery | [`tests/btech_crew.rs`](../../tests/btech_crew.rs), [`tests/btech_recovery.rs`](../../tests/btech_recovery.rs), power admission tests in [`src/btech/power.rs`](../../src/btech/power.rs) |
| Weapon arcs, readiness and recycle | [`tests/btech_arcs.rs`](../../tests/btech_arcs.rs), [`tests/btech_vehicle_arcs.rs`](../../tests/btech_vehicle_arcs.rs), [`tests/btech_vehicle_aim.rs`](../../tests/btech_vehicle_aim.rs) |
| Ammunition and feed exhaustion | [`tests/btech_vehicle_ammunition_cascade.rs`](../../tests/btech_vehicle_ammunition_cascade.rs), [`tests/btech_weapon_failure.rs`](../../tests/btech_weapon_failure.rs), [`tests/btech_vehicle_unjam.rs`](../../tests/btech_vehicle_unjam.rs) |
| Unit and vehicle destruction cleanup | [`tests/btech_vehicle_explosion.rs`](../../tests/btech_vehicle_explosion.rs), [`tests/btech_vehicle_flood_state.rs`](../../tests/btech_vehicle_flood_state.rs), [`tests/btech_vehicle_internal_damage.rs`](../../tests/btech_vehicle_internal_damage.rs) |
| Lua API contract and observation boundary | [`tests/lua_autopilot.rs`](../../tests/lua_autopilot.rs), [`tests/lua_tactical.rs`](../../tests/lua_tactical.rs), this audit |

The matrix records where each concern is exercised without duplicating the
larger combat suites in this focused audit.

Run the focused scenarios through the existing `btech_01` suite:

```text
cargo test --test btech_01 btech_autopilot_audit -- --nocapture
```

Projection uses the admitted firing mode, including ammunition-dependent burst
fallback and enhanced weapon heat damage. A gatling launch reserves its maximum
possible heat without rolling dice in the decision phase. The focused runtime
unit regression covers normal, burst, damaged, and gatling projections.

## Verification result

On 2026-09-22, `cargo fmt` and `just checks` completed successfully, including
Lua type/documentation freshness checks and the full `cargo test` suite:
3,010 tests passed, zero failed, and one existing pinned-C differential probe
was ignored because it requires separately built C and Rust server binaries.
The long production-heartbeat measurements are recorded separately in
[`autopilot-benchmark.md`](../../benchmarks/autopilot/autopilot-benchmark.md).

## Fast behavior-test execution

Run all 18 order, runtime, and gameplay-audit scenarios with:

```text
cargo test --test btech_01 btech_autopilot
```

On the documented development machine, three runs of the prebuilt test binary
completed in 1.512, 1.434, and 1.512 seconds, compared with 8.191 seconds before
this fixture optimization. These figures exclude compilation and Cargo startup;
they are measurements, not wall-clock assertions in the tests.

Order and audit traces use the production `HeartbeatHarness`, assert that every
step commits, inspect the live committed world, and reload durable controller
state once at the end. Sensor sightings are deliberately excluded from that
restart comparison. Behavior-based stopping conditions replace unused trailing
ticks, and short routes retain the relevant movement and turning transitions.
Patrol now requires the waypoint index to wrap; the competing-route scenario
requires both controllers to complete their distinct goals. Negative firing
checks retain fixed multi-tick observation windows.

The move-to-hold scenario still exercises the scheduled server heartbeat. The
restart and injected-failure scenarios retain their real SQLite paths. Neither
persistence nor simulation rules are disabled, and the production benchmark's
workload and timing results are unchanged by these test-only changes.
