# Autopilot observation and combat CPU work

This pass preserves the one-second observation cadence, three-tick target
reassessment, mount order, ordinary shot admission, and the live heat/ammunition
checks after every attempted shot. It adds no Lua API or persistence schema.

## Implementation boundaries

Acquired contacts share one admitted observer and one lazy illumination context.
The context borrows the world immutably, prepares active light/inferno candidates
once, and shares target illumination between sensor roles. It is discarded
before any world mutation; neither another controller's observation nor a later
shot can reuse stale visibility. Contact display and autonomous observations
continue to use the same contact eligibility function. Clairvoyance does not
expand the autopilot's acquired-contact list.

LOS memoizes only directed center-to-center **hex sequences**. Cache keys are the
four endpoint coordinates; values contain no map identity, terrain, height,
unit state, visibility or light. Every LOS query still evaluates its live map
and endpoint heights. Direction is part of the key because reversing a trace
could change floating-point boundary ownership. The per-thread FIFO cache is
bounded by 32,768 entries and 1,000,000 retained coordinates (8 MB of coordinate
payload, plus bounded container overhead); oversized sequences are not retained.
The engine currently runs the heartbeat on one thread. Benchmark repetitions
clear the cache, and each tick asserts both retention bounds separately from
A* search-memory limits. Reported cache peaks are sampled retained counts, not
whole-process memory measurements.

Target selection prepares ready weapon profiles once and scores each eligible
contact once, retaining the same summation order, ties and hysteresis. Weapon
preparation shares a resolved loadout for readiness, damage and supply fallback;
ordinary mechanics remain the authority. Immutable admission and validation
scopes also share parsed equipment. Registered source addresses are identity
tokens only (never dereferenced); lifetime-bound guards keep sources immutable,
reject cloned objects at different addresses, and remove projections on drop.
These scopes cannot move across threads. Cache storage is bounded by the units
registered in the current read, and no loadout scope crosses a mutation or shot
attempt.

## Reproducible comparisons

The benchmark seeds map fire, attack, sensor-fluctuation and crew-recovery dice
with separate deterministic streams. Earlier benchmarks seeded only map fire
and attack dice. Seeding the other streams makes complete state comparisons
possible without excluding dice from the checksum; it does not disable any rule.
The historical report remains available in `autopilot-benchmark.md`.

```sh
# Short comparison, run on both baseline and candidate release executables.
autopilot-bench --warmup 35 --ticks 60 --repetitions 3 --trace trace.jsonl

# Inclusive attribution, run separately from acceptance timing.
autopilot-bench --warmup 35 --ticks 5 --repetitions 1 --detailed

# Full acceptance: 100 controllers, all six cases, 30 warmup/600 measured ticks,
# five repetitions. Detailed attribution and gameplay tracing are disabled.
cargo run --release --bin autopilot-bench
```

`--trace` writes one JSONL record per committed tick, including warmup. A stable
FNV-1a/128 checksum covers serialized BattleTech state (including orders,
feedback, unit effects and all persisted simulation dice) and the autopilot's
ordered movement/combat notices. It excludes wall-clock measurements and
transient planning caches. This is a regression checksum, not a security hash.
Serialization runs outside measured heartbeat phases. A failed heartbeat clears
its optional autonomous notice trace along with ordinary rollback.

`--detailed` reports inclusive nanosecond totals and call counts in this order:
contacts, geometry, sensors, illumination, readiness, selection, shots. Nested
categories overlap and must not be added. Baseline contact calls are scalar;
the candidate's contact calls are batches, so their call counts are not directly
comparable. Instrumentation is scoped to synchronous autopilot work, never an
async suspension. Disabled attribution performs no per-call clock reads.

The existing CSV columns retain their meanings; appended columns report movement
and combat p95 and the bounded topology cache's sampled retained counts. Percentiles
of sub-phases cannot be added to reconstruct the enclosing phase percentile.

## Validation and measurements

`cargo fmt` and `just checks` passed, including Lua type/documentation freshness
checks and all 3,016 Rust tests. One existing pinned-C differential test remains
ignored because it needs separately built reference binaries. New regressions
cover batch/scalar contact parity, live illumination changes, generated target
selection, directed topology equivalence and eviction limits, terrain/posture
changes on warm traces, loadout-scope lifetime/clone boundaries, and rollback of
optional diagnostic notices.

The paired short comparison uses 35 warmup and 60 measured ticks across three
repetitions of each case. All **1,710 committed tick records matched byte for
byte**, including warmup. Their JSONL SHA-256 is
`b2bf256932a14e11403374a8ea75547ff2bbc0054e88f60d37f9073b180fcc8b`.
Raw results are in [baseline CSV](autopilot-cpu-baseline-short.csv) and
[candidate CSV](autopilot-cpu-candidate-short.csv). Inclusive attribution totals
and call counts are in [profile JSON](autopilot-cpu-profile.json).

| Case | Baseline autopilot p95 (ms) | Candidate p95 (ms) | Reduction |
| --- | ---: | ---: | ---: |
| open / hold | 59.936 | 25.683 | 57.1% |
| open / fire | 111.216 | 55.881 | 49.8% |
| obstacles / hold | 71.482 | 31.741 | 55.6% |
| obstacles / fire | 125.188 | 65.107 | 48.0% |
| moving congestion / hold | 60.455 | 26.101 | 56.8% |
| moving congestion / fire | 121.314 | 61.581 | 49.2% |

These short-run figures are comparisons, not the full acceptance result. The
50 ms target was met in the three weapons-hold cases and missed in the three
firing cases. No work or game rules were disabled to obtain the reduction.

The baseline executable SHA-256 is
`b9ffe95dcc05fe1b56f33c596fc123a23f204c11cabe3410b8e3b63f544205a9`;
the candidate is
`236334cf30c5a93f8d136a4624ce22f51367f5b592515b0e4827c92377738455`.
Both use rustc 1.98.1 and the normal release profile on the documented Intel
Core i7-10875H / 32 GB Linux development machine. Builds and tests finished
before timed comparisons began; baseline and candidate ran sequentially.

## Full acceptance

The complete default protocol finished successfully in **60.4 minutes** (3,624.30
seconds): 30 independent repetitions, 900 warmup ticks, and **18,000 measured
production heartbeats**. Each row aggregates 3,000 measured ticks. Tracing and
detailed attribution were disabled. All 100 enabled controllers were serviced
every tick, every heartbeat committed, and all per-controller/global expansion,
search-record, and topology-cache assertions passed.

Times are milliseconds. The [full CSV](autopilot-cpu-full.csv) includes medians,
maxima, persistence timings, and all workload counters.

| Case | Autopilot p95 | Observation p95 | Navigation p95 | Combat p95 | Whole heartbeat p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Open / hold | 26.8 | 21.4 | 0.4 | 21.7 | 189.2 |
| Open / fire | 54.2 | 26.0 | 0.5 | 47.2 | 249.3 |
| Obstacles / hold | 32.0 | 20.7 | 5.6 | 21.0 | 207.7 |
| Obstacles / fire | 62.4 | 25.5 | 6.9 | 49.4 | 294.9 |
| Congestion / hold | 29.1 | 22.3 | 0.8 | 22.7 | 196.5 |
| Congestion / fire | 59.4 | 27.2 | 1.0 | 50.5 | 258.2 |

**The 50 ms autopilot p95 target is met in the three weapons-hold cases and
missed in the three firing cases.** The full-run range is 26.8–62.4 ms.
Observation p95 is now 20.7–27.2 ms. Combat p95 includes observation and shot
work; these nested phase percentiles must not be added. The remaining timing
gap warrants further profiling of firing work, including ordinary admission and
validation, without relaxing gameplay checks.

| Case | Expansions | Peak retained search records | Replans | Completed orders | Completion latency p95 (ticks) |
| --- | ---: | ---: | ---: | ---: | ---: |
| Open / hold | 56,610 | 0 | 2,965 | 625 | 425 |
| Open / fire | 61,969 | 349 | 3,038 | 556 | 425 |
| Obstacles / hold | 8,221,355 | 95,722 | 5,046 | 206 | 561 |
| Obstacles / fire | 8,347,600 | 90,335 | 5,126 | 175 | 558 |
| Congestion / hold | 236,945 | 978 | 3,757 | 573 | 593 |
| Congestion / fire | 253,261 | 2,024 | 3,843 | 506 | 562 |

The measured workload admitted **16,311 autonomous shots**, completed
**2,641 orders**, and recorded 23,775 replans,
1,978 blocked transitions, and 2,180 between-tick
restorations. These stress fixtures restore destroyed or terminally blocked units
to retain the active load, so completion counts are not ordinary scenario success
rates. The maximum sampled retained search frontier was 95,722 records; completed
frontiers are already released at sampling, so zero does not mean zero allocation.
The topology cache reached its 32,768-entry cap and at most 890,560 retained
coordinates. Neither counter measures whole-process resident memory.

The full candidate run validates sustained workload and bounds. Gameplay parity
is established by the separate seeded paired short comparison above; the
historical full run used different sensor/recovery seeds and is not a paired
outcome-equivalence comparison. Wall time remains a measurement on this development
desktop, not a deterministic assertion.
