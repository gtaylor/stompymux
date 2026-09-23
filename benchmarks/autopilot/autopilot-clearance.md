# Responsive congestion recovery

Ground autopilots retain a bounded watch of cells rejected for excessive known
occupancy. When a watched cell becomes enterable, they can start a new search
without waiting for the next 30-second retry. This changes neither stacking
admission nor hidden-contact rules.

## Retry policy

- Each search accumulates at most 64 unique crowded destination cells, keeping
  the lexicographically smallest coordinates regardless of discovery order.
- Waiting controllers check those cells every five simulation seconds, using
  one filtered occupancy collection. Unrelated traffic and reductions that
  leave the cell crowded do not qualify.
- Three early retries supplement three timed retries. Attempts are charged only
  when search-record admission succeeds. A timed deadline takes precedence over
  a clearance trigger; an early failure never postpones that deadline.
- Exhausted frontiers are released; waits perform no A* expansions. Search
  quotas and the rotating controller scheduler remain authoritative.
- After the third timed search fails, ordinary fallback/blocking applies.
  Watches reset on successful routing and planning-context invalidation.
  Replacing an order with a stationary intention also invalidates stale work.
- Watches, deadlines, eligibility and counters are transient. Restart rebuilds
  planning; failed commits discard transient work and diagnostic attribution.

The watch bound can miss a useful clearance when more than 64 crowded cells
were encountered. Timed retries remain the fallback. Resource pressure can
also delay an eligible search; the five-second bound assumes available budget.
There is no passage reservation, automatic resumption of blocked orders, or
change to hostile contact expiry.

## Encounters and measurements

The established suites retain their default matrices. New scenarios require an
explicit selection:

```sh
cargo build --release --bin autopilot-encounters --bin autopilot-bench
for scenario in late_clearance alternating_clearance waiting_controllers; do
  target/release/autopilot-encounters --suite adversarial --scenario "$scenario" \
    --ticks 240 --seeds 3 --trace "$scenario.jsonl" > "$scenario.json"
done
```

`late_clearance` schedules ordinary blocker movement so the first relevant
cell clears just after the timed retry at tick 31. Release times differ by
chassis to accommodate their ordinary turning and acceleration.
`alternating_clearance` reverses blocker orders and releases them again.
`waiting_controllers` uses three independent parallel corridors on one map;
it does not ask three Mechs to finish in one stacking-limited destination.

Optional `congestion` participant summaries contain first watched-cell
clearance, clearance-to-search and clearance-to-movement delays, poll counts,
early/timed starts and resource deferrals. Clearance is sampled independently
before the heartbeat, using the same permitted occupancy view as navigation.
Movement means more than 0.02 hexes in a committed tick. The preexisting
`clearance_tick` (all blockers beyond the destination) and `recovery_ticks`
are preserved and measure a different event.

The opt-in runtime-only 100-waiter benchmark isolates polling cost from SQLite,
combat and ordinary motion. It compares the same crowded world with early
polling disabled/enabled, asserting all 100 controllers receive service and
no search expansions occur during the sampled waits:

```sh
cargo test --release --lib waiting_controller_polling_benchmark -- --ignored --nocapture
```

Full protocol, measurements, replay evidence and remaining limitations are
recorded in [the artifacts](autopilot-clearance-artifacts/README.md).

## Measured late-clearance improvement

All 12 late-clearance cases (four chassis × three seeds) passed the comparison.
The watched cell becomes enterable at tick 33 in both implementations. The
candidate starts searching at tick 36 instead of tick 61: **25 simulation
seconds sooner**, reducing clearance-to-search delay from 28 to 3 ticks.

| Chassis | Clearance-to-movement, before → after | Completion tick, before → after |
| --- | ---: | ---: |
| Mech | 30 → 5 | 130 → 105 |
| Tracked | 32 → 7 | 175 → 150 |
| Wheeled | 32 → 7 | 175 → 150 |
| Hover | 32 → 7 | 175 → 150 |

All three seeds give these results. Movement latency includes ordinary turning
and acceleration; the five-second guarantee is for search eligibility/service
with available resources, not immediate physical movement.

## Recovery and lifecycle regressions found by the fixtures

The reversing-traffic encounter exposed a readiness mismatch: a successful stand
sets posture to standing before its rising countdown finishes. Autopilot then
attempted movement, received an ordinary admission rejection, and incorrectly
blocked the order as `UnitUnavailable`. Movement planning now waits for the
existing stand timer to finish; it does not bypass admission or consume dice.
This also affects duels in which a Mech falls and stands, so those trace changes
are reviewed as recovery changes rather than classified as unrelated output.

Unplacement testing also exposed retained transient plans. Battlefield departure
now clears the plan for both chassis classes, including administrative removal
and map transfer. Durable orders and their ordinary validation remain unchanged.

Initial traced runs used normal temporary storage. Remaining correctness-only
replays use RAM-backed temporary databases to reduce I/O wait while retaining
real SQLite transactions and commits. No timing claims are derived from those
runs. The CPU regression guard uses the normal filesystem, matching its frozen
reference; injected persistence-failure tests use disposable fixture databases.

The corrected alternating-clearance matrix passes and replays identically.
Its previously blocked Mech traffic helper remains active at the 240-tick
cutoff, rather than failing during a successful rise. Some reversing traffic
helpers do not finish within that window; all measured focal orders do. These
helper outcomes remain visible in the raw summaries. This is not a guarantee
of coordinated passage or completion for arbitrary reversing traffic.

## Correctness acceptance

The final code passed `just checks`: **3,053 tests passed, two ignored, across
21 groups**, including Lua type/doc and formatting checks. The focused autopilot
suite passed 42 tests; the ignored polling benchmark is run separately. Ten
Python comparator tests passed.

All **264 encounters** passed their gates and their **63,360 committed tick
records replayed byte for byte**: 120 existing movement cases, 108 standard
adversarial cases and 36 additional clearance cases. These are two full
final-build matrices, not an assembly of partial runs.

The existing suite's 28,800 records also match the prior implementation exactly.
Another 17,280 adversarial records outside congestion and duel recovery cases
match the prior trace exactly. Congestion differences are intentional (including
new diagnostic fields). Duel differences were reviewed: all eight prior
`UnitUnavailable` blocks disappear after the rising-timer fix. Across both duel
participants, destroyed/completed outcomes increase from four each to nine each;
six participants remain active at the window end. This restores continued combat,
not a claim of improved survival or win rate.

## Isolated 100-waiter polling cost

On the documented development machine, 500 measured runtime samples per mode
(no concurrent builds, tests or encounters) gave:

| Mode | Movement-runtime p50 | Movement-runtime p95 |
| --- | ---: | ---: |
| Early polling disabled | 0.447 ms | 0.498 ms |
| Early polling enabled | 0.450 ms | 2.211 ms |

The p95 on ticks that actually poll is 2.222 ms. This 1×8-hex corridor fixture has four stationary blockers and deliberately
synchronizes all 100 watchers, includes optional per-unit diagnostic collection,
and excludes SQLite, combat and ordinary motion. The burst adds about 1.71 ms
to its overall p95 while leaving the median nearly unchanged. Every sampled
wait asserts 100 serviced controllers and zero search expansions. These numbers
are distinct from the six-case whole-heartbeat regression guard.

## Six-case CPU guard

The original frozen release benchmark and final candidate ran sequentially on
the normal filesystem: 100 controllers, 100×100 maps, 35 warmup ticks,
60 measured ticks and three repetitions per case (**1,080 measured heartbeats
per executable**). Tracing and detailed attribution were disabled; no builds,
tests or encounter runs overlapped these measurements.

| Scenario | Fire | Autopilot p95, before → after (ms) | Whole-heartbeat p95, before → after (ms) |
| --- | --- | ---: | ---: |
| open | hold | 36.33 → 35.94 | 181.40 → 170.67 |
| open | opportunistic | 64.05 → 60.19 | 227.18 → 217.54 |
| obstacles | hold | 41.00 → 39.43 | 189.12 → 184.03 |
| obstacles | opportunistic | 63.94 → 62.85 | 234.73 → 230.02 |
| moving_congestion | hold | 34.86 → 35.48 | 168.66 → 169.33 |
| moving_congestion | opportunistic | 57.22 → 55.88 | 214.68 → 218.14 |

All cases satisfy the **75 ms firing / 50 ms weapons-hold guards**. The largest
hold p95 increase is 1.79%, below the 5% investigation threshold. Resource,
commit and controller-service assertions passed. All cases retained 100 enabled
controllers and a maximum service delay of one tick; search expansion and
record limits remained enforced.

Firing workload counters change after corrected stand recovery: fewer orders
block and require renewal, and shot totals differ. The comparison JSON lists
these differences. Lower timings are therefore not claimed as a pure CPU
optimization or gameplay-equivalent throughput gain. Whole-heartbeat p95 fell
in four cases; moving-congestion p95 rose by 0.67 ms with weapons held and
3.46 ms while firing. Raw CSV also retains persistence timings.
This short regression guard does not replace full-duration performance acceptance.
