# Ground autopilot heartbeat benchmark

Run the isolated production-heartbeat workload from the crate root:

```text
cargo run --release --bin autopilot-bench
```

The default run evaluates six cases: open, obstacle-heavy, and moving-
congestion maps, each with weapons held and opportunistic direct fire. Every
case uses five independent repetitions, 30 warmup heartbeats, 600 measured
heartbeats, and 100 attached controllers split evenly between a Mech, tracked,
wheeled, and hover chassis. A fixed seed makes map generation and the fixture
repeatable. Use `--ticks 5 --warmup 35 --repetitions 1` for an active-workload smoke run; fewer than 30 startup ticks cannot measure autonomous movement or fire.

Each repetition creates a temporary copy of the fixture game root, initializes a
temporary SQLite database with the fixture world, and drives
`HeartbeatHarness`. The harness runs the actual server heartbeat, including
the transactional commit and rollback boundary. It never opens the live game
database. The CSV columns report p50, p95, and maximum values across measured
ticks:

* `p50_*`, `p95_*`, and `max_*` are the median, 95th percentile, and maximum
  samples for each timing family. `p95_heartbeat_ms` includes the complete
  production heartbeat and commit.
* `p95_autopilot_ms` is the movement-decision plus combat-decision phase,
  including controller scheduling and resource accounting.
* `p95_navigation_ms` and `p95_observation_ms` are measured sub-phases of the
  autopilot decision work; they are recorded in the same heartbeat and are not
  estimated by subtracting separate benchmark runs.
* `p95_persistence_ms` is the validated SQLite persistence commit.
* `expansions`, `max_controller_expansions`, `peak_search_records`, `replans`,
  `completed`, `blocked`, and `shots` are runtime counters. `peak_search_records`
  samples retained frontiers after each controller is serviced; completed
  frontiers are already released, so this is not peak search allocation. Search
  budgets and the one-million-record ceiling remain enforced by production
  admission and navigation, with benchmark assertions.

The acceptance target is p95 autopilot-only service below 50 ms on the
documented development machine. The benchmark prints observed measurements;
it does not substitute a navigation-only estimate or claim a target was met
without a run.

The workload retains 100 enabled controllers before every tick. Units that are
destroyed or terminally blocked are restored from the fixture between ticks and
receive replacement orders through the ordinary controller engine. Their order
IDs and feedback sequence remain monotonic. The `renewals` column makes this
scenario intervention explicit; renewal time is outside the measured heartbeat.
Without renewal, an inactive battlefield would dilute the timing results. The
`min_enabled` and `max_service_delay_ticks` columns accompany assertions that
every controller is serviced every tick under the production expansion limits.

Each controller alternates finite movement orders before a continuing patrol.
Completion latency is measured from the matching order-started event to its
successful completion, in simulation ticks; missing completions are reported
as `-`, never as zero. At measured ticks 300 and 330 the scenario adds and then
removes a terrain barrier, exercising live route invalidation. Maps use actual
wall terrain for obstacle and congestion barriers. Ordinary movement, sensors,
weapon admission, damage, and SQLite commits remain enabled.

## Profiling and optimization

Initial active-load stack sampling identified repeated illumination queries as
the dominant cost: each sensor pair scanned every unit and built scanner views
for inactive searchlights. The implementation now filters live lamp/inferno
state before geometry, avoids equipment parsing for dark lamps, and skips system
name parsing when a slot is intact. Undamaged units have a direct zero-loss
system check. These changes preserve authoritative illumination and damage
rules; they do not cache decisions across movement or combat.

Autopilot observations now share acquired-contact facts with the ordinary
contact display, without producing display strings, weapon arcs, names, or
redundant condition text. Visibility checks and range/identity ordering remain
shared. Navigation also builds known occupancy once per bounded search step.
The whole-heartbeat harness measures the production path, including those
shared engine improvements, rather than a copied benchmark implementation.

A subsequent profile found repeated LOS traces in primary sensor, secondary
sensor, and identification queries. Contact facts now reuse geometry within a
single read-only observation, without retaining it across world mutations.
Non-pursuing orders obtain their contact snapshot once in the post-movement
phase, and readiness inspection shares one resolved loadout per unit. Hex
containment uses squared distances away from ties, with the original Euclidean
boundary rule near ties; a generated oracle regression covers both random
points and exact/near boundaries. All mechanical and sensor eligibility checks
remain in the ordinary execution path.

## Measurement machine

The acceptance run uses an Intel Core i7-10875H (8 cores, 16 logical CPUs,
2.30 GHz nominal), 32 GB RAM, Linux 7.1.5-76070105-generic x86_64, and
`rustc 1.98.1 (48a229cea 2026-09-01)`. The executable is built with the normal
Cargo release profile. Cases run sequentially after compilation and regression
tests finish. This is a development desktop, not a dedicated benchmark host;
wall-clock timings describe this run rather than a deterministic guarantee.

## Acceptance results — 2026-09-22

The complete default protocol finished successfully: 30 independent repetitions,
900 warmup ticks, and 18,000 measured production heartbeats. Each row below
aggregates 3,000 measured ticks from five repetitions. All 100 enabled controllers
were serviced every tick; every heartbeat committed. The per-controller expansion,
global expansion, and global search-record assertions remained satisfied.

Times are milliseconds. See the [complete CSV](autopilot-benchmark-results.csv)
for medians, maxima, sub-phase timings, and all workload counters.

| Case | Autopilot p95 | Navigation p95 | Observation p95 | Whole heartbeat p95 | Persistence p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Open / hold | 66.4 | 0.8 | 61.4 | 305.4 | 78.0 |
| Open / fire | 109.8 | 0.8 | 66.6 | 374.5 | 72.8 |
| Obstacles / hold | 73.1 | 7.9 | 60.8 | 331.3 | 73.4 |
| Obstacles / fire | 118.2 | 8.9 | 66.9 | 412.0 | 74.4 |
| Congestion / hold | 69.1 | 1.2 | 62.7 | 313.8 | 71.9 |
| Congestion / fire | 114.7 | 1.4 | 69.2 | 384.4 | 73.3 |

| Case | Expansions | Peak retained search records | Replans | Completed orders | Completion latency p95 (ticks) |
| --- | ---: | ---: | ---: | ---: | ---: |
| Open / hold | 56,610 | 0 | 2,965 | 625 | 425 |
| Open / fire | 62,113 | 349 | 3,028 | 553 | 426 |
| Obstacles / hold | 8,221,355 | 95,722 | 5,046 | 206 | 561 |
| Obstacles / fire | 8,339,248 | 90,335 | 5,102 | 174 | 558 |
| Congestion / hold | 236,945 | 978 | 3,757 | 573 | 593 |
| Congestion / fire | 253,579 | 2,024 | 3,871 | 503 | 562 |

**The 50 ms autopilot p95 target was not met in any case.** The measured
range was 66.4–118.2 ms.
The implementation satisfies the deterministic work and storage bounds, but
further runtime optimization is needed to meet the timing target on this machine.
Navigation p95 stayed below 9 ms, while observation p95 alone exceeded 50 ms in
every case. These measurements point to observation and combat work as the next
optimization area; replacing the pathfinder alone cannot meet this target.

The measured workload admitted 16,350 autonomous shots and
completed 2,634 orders. It recorded
1,984 blocked transitions and used
2,185 between-tick restorations to retain the active load.
Restorations include destruction and terminal blocking; these are stress fixtures,
so completion counts are not a success rate for ordinary scenarios. Per-case
counts remain in the CSV. The retained-record counter samples frontiers after
each controller is serviced. Completed frontiers have already been released, so
zero means no retained frontier at those boundaries, not zero search allocation.
It is neither a peak-allocation measurement nor whole-process resident memory.
Each admitted job also has a reserved record limit, whose global sum cannot
exceed one million. Percentiles of separate phases need not add to the
percentile of their enclosing heartbeat.

Validation before measurement: `just checks` passed, including Rust/Lua formatting,
Lua type/documentation freshness checks, and 3,010 passing tests. One existing
pinned-C differential probe remained ignored.

## Subsequent CPU optimization

See [observation and combat CPU measurements](autopilot-cpu.md) for the subsequent
implementation, seeded baseline comparison, outcome-equivalence checks, and full
acceptance results. The measurements above preserve the historical baseline.
