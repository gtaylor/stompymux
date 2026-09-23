# Autopilot shot admission and validation CPU work

This pass targets the remaining 50 ms autopilot p95 gap from
[the observation/combat optimization report](autopilot-cpu.md). It preserves
observation cadence, target selection, mount order, ordinary admission, dice,
per-shot atomicity, and the production heartbeat/persistence boundary.

**The full timing target remains unmet:** firing p95 is now 46.324 ms open,
55.675 ms obstacles, and 52.222 ms congestion. Correctness and resource gates pass;
obstacle and congestion firing still exceed 50 ms.

## Changes and safety boundaries

- Shot admission registers only its two participants in the immutable equipment
  context. Other lookups retain their ordinary uncached behavior.
- Direct Mech/vehicle salvos can borrow an explicit shot-owned candidate. Their
  callers propagate all errors to that owner. Standalone salvos and the Swarm
  path retain independent savepoints. Validation still occurs at its existing
  boundaries; a volley is not an atomic batch.
- During synchronous autonomous combat, successful local Mech and map validation
  may be reused only when the complete saved value compares equal. The cache
  contains at most one prior value for each Mech/map in the phase's starting
  roster; new identities cannot grow it. Cross-object checks still execute.
- A validated unit's equipment projection can seed the next immutable admission
  context or serve mount preparation, again only after full value equality.
  Heat, ammunition, damage, readiness, and targeting remain live per attempt.
- Cache setup, lookup, replacement, and teardown are inside combat timing. Scope
  exit discards all entries; dropping a failed shot candidate clears them. Nothing
  is persisted or retained between heartbeats. Failed database commits occur after
  the synchronous context has already been discarded.

`BattleUnit` and `StoredBattleMap` use derived value equality. Their validation
inputs are owned values or immutable Arc contents, with no mutable shared cells.
The custom dice equality excludes only generic-roll diagnostics, which neither
local validator reads. NaN values cannot qualify for equality reuse. The full
BattleTech state, world objects, and relationship validators are never cached.
The broader state contains runtime mutable cells, which are deliberately outside
this cache's inputs and scope.

Pure equipment resolution also has a combat-local cache keyed by the entire
owned template and contract-mode flag, exactly the inputs read by its parser.
This applies to both Mechs and vehicles and shares equal template inputs across
units. Live heat, ammunition, critical availability and readiness are not parser
inputs and remain uncached. Separate FIFO limits equal the number of Mechs and
vehicles at scope entry, and an assertion guards every insertion. Failed parses
are not retained. Scope exit and failed outer shots clear these projections.

An experiment caching complete vehicle-local validation values and preparing
contact-position lookups was rejected: paired obstacle/fire p95 increased from
57.0 to 62.9 ms. The associated controller-ID stack-buffer experiment was also
removed. Vehicle validation still follows its original path. A subsequent
bounded dense array improved contact lookup: within each validation call it
indexes current positions only when the ID span is at most 65,536 and at most
four times the unit count. Sparse or extreme IDs retain tree lookup. This is
not a validation certificate: every contact check still runs in the same order,
including missing/unplaced identities and vehicle-first duplicate precedence.

No Lua API or persistence schema changes are needed. Public standalone firing and
damage operations retain their transactional semantics. A test-only reference
mode keeps nested salvo savepoints and disables validation reuse for comparison.

## Diagnostics and reproduction

The existing seven diagnostic categories and CSV columns retain their meaning.
`--detailed` additionally emits a named `combat` object with call counts and
nanoseconds for mount preparation, admission/aim, candidate creation,
copy-on-write detachments, launch/defenses, damage/recoil, validation, and
publication. Counters distinguish attempts, rejections, successes, nested
transactions, and reused validation/equipment results.

Timings are inclusive. `validation` measures complete BattleTech validation;
`validation_unit` also includes local validation in admission and damage.
`validation_cross_object` is complete validation time minus its local unit/map
measurements, so it includes validation framework overhead. Do not add these
nested categories. Copy-on-write attribution covers the instrumented shot,
launch, salvo, and impact entry points, not every allocation in the program.
Disabled diagnostics perform no per-call clock reads or diagnostic allocations.

```sh
# Full protocol remains the default; filters change only which cases run.
autopilot-bench --scenario obstacles --fire opportunistic \
  --warmup 35 --ticks 60 --repetitions 1 --detailed

# Paired outcome comparison: execute sequentially on baseline and candidate.
autopilot-bench --warmup 35 --ticks 60 --repetitions 3 --trace trace.jsonl

# Full acceptance: six cases, 100 controllers, 100x100 maps,
# five repetitions, 30 warmup and 600 measured ticks.
autopilot-bench
```

Scenario choices are `all`, `open`, `obstacles`, and `moving_congestion`; fire
choices are `all`, `hold`, and `opportunistic`. Both default to `all`. Unknown
values are rejected. Tracing and detailed attribution are off for acceptance.

## Baseline and profiling

The starting source is commit `1dce945ed34c6f62dca5c510c44242e25a4cb3cf`, with
an empty working diff. Its saved release executable SHA-256 is
`236334cf30c5a93f8d136a4624ce22f51367f5b592515b0e4827c92377738455`.
The executable, source archive/diff, compiler version, existing reports, traces,
and intermediate measurements were preserved under `/tmp/autopilot-combat-cpu`.

The fresh six-case baseline measured firing p95 of 66.0–79.6 ms, slower than the
previous report's 54.2–62.4 ms. This is a development desktop, not an isolated
benchmark host. Comparisons use the fresh baseline rather than claiming the
historical wall times are interchangeable. Its 1,710 tick records match the
previous seeded trace byte for byte.

An obstacle/fire attribution window (35 warmup, 60 measured ticks) recorded
7,562 attempts, 7,462 rejections, and 100 accepted shots. This highlighted repeated
admission and validation work. A separate 80-sample GDB run corroborated loadout,
validation, and cloning activity; debugger runs are not acceptance timings.

Intermediate paired obstacle/fire p95 results were 79.6 ms baseline, 74.9 ms
with participant registration, 78.3 ms with shared salvo candidates, and 58.2 ms
with local validation reuse. Every intermediate 285-tick trace matched. The
transaction-only p95 change was noisy; separate attribution showed 37 nested
candidates removed, instrumented copy-on-write events reduced from 137 to 101,
and damage/recoil time reduced from 39.1 to 28.4 ms across the measured window.
The larger improvement came from validation reuse, not simply fewer world clones.

The pure equipment cache subsequently reduced obstacle/fire p95 to 55.5 ms.
The bounded contact index measured 55.3 ms, with cross-object attribution dropping
from 99.2 to 81.3 ms across the same 60-tick profile. Both 285-record traces
matched the baseline. Detailed final attribution recorded 30,967 equipment
projection reuses and 240 actual resolutions.

Final detailed attribution, accumulated over the same 60 measured obstacle/fire
ticks (milliseconds, inclusive):

| Subcategory | Before | After |
|---|---:|---:|
| `mount_preparation` | 43.74 | 18.47 |
| `admission_aim` | 210.99 | 107.78 |
| `candidate_creation` | 3.50 | 2.41 |
| `copy_on_write` | 28.44 | 14.17 |
| `launch_defenses` | 118.72 | 65.30 |
| `damage_recoil` | 39.11 | 15.60 |
| `validation` | 187.86 | 91.13 |
| `validation_cross_object` | 107.35 | 81.34 |
| `publication` | 15.85 | 9.44 |

Both profiles admitted 100 shots and rejected 7,462 attempts. Full state
validation still ran 175 times; only proven local checks were reused. The
remaining `contacts` attribution is 1,433.8 ms versus 313.8 ms for the existing
`shots` category. These are inclusive measurements, not additive totals; the
profile now ranks sensor/contact work ahead of shot execution.

## Paired short-run results

Both executables ran sequentially with 35 warmup and 60 measured ticks, three
repetitions per case. All 1,710 committed records, including warmup, were byte
identical (SHA-256 `b2bf256932a14e11403374a8ea75547ff2bbc0054e88f60d37f9073b180fcc8b`).
The table reports p95 milliseconds; full acceptance is a separate longer run.

| Scenario | Fire | Baseline autopilot | Candidate autopilot | Baseline heartbeat | Candidate heartbeat |
|---|---|---:|---:|---:|---:|
| open | hold | 31.932 | 26.524 | 227.726 | 192.147 |
| open | opportunistic | 66.007 | 44.510 | 275.801 | 232.625 |
| obstacles | hold | 40.851 | 32.637 | 247.061 | 205.339 |
| obstacles | opportunistic | 79.631 | 56.138 | 299.258 | 250.321 |
| moving_congestion | hold | 33.691 | 26.667 | 235.299 | 190.261 |
| moving_congestion | opportunistic | 70.857 | 48.030 | 286.684 | 225.311 |

Whole-heartbeat times improved as well: the change did not merely move work
outside the measured autopilot phases. No hold case regressed in this comparison.

## Final validation and acceptance

`cargo fmt` and `just checks` passed. Lua type/documentation freshness checks
passed, followed by 3,027 Rust tests; one existing pinned-C reference-binary test
remains ignored. New deterministic tests cover nested/flattened shot parity,
Mech/tracked/wheeled/hover shooters against both target classes, successful and
rejected mounts, assigned-pilot manual firing, critical/destruction outcomes,
Inferno, vehicle Flamer heat, Heavy Gauss recoil, and all four AMS chassis pairs.

Injected expenditure, damage and validation failures restore complete serialized
world state and generic-roll journals, and discard validation/equipment records.
A new integration test rejects a firing heartbeat's database commit and verifies
no state or notices escape; retry matches the successful control heartbeat.
Mutation sequences compare cached and full validation for map geometry, invalid
units, crew relocation, contact references, towing and registration changes.
Separate tests cover parser inputs/contract mode, eviction, cache scope exit,
sparse/extreme IDs and duplicate/unplaced contact identities.

The complete acceptance run finished in **3,629.40 seconds (60.49 minutes)**:
100 controllers on 100×100 maps, all six cases, five repetitions, 30 warmup and
600 measured ticks. This is 18,000 measured heartbeats plus 900 warmup heartbeats.
Tracing and detailed attribution were disabled; no builds or tests ran alongside it.

| Scenario | Fire | Autopilot p95 ms | Whole-heartbeat p95 ms | Below 50 ms |
|---|---|---:|---:|---|
| open | hold | 27.416 | 200.003 | yes |
| open | opportunistic | 46.324 | 243.582 | yes |
| obstacles | hold | 32.429 | 207.776 | yes |
| obstacles | opportunistic | 55.675 | 284.731 | **no** |
| moving_congestion | hold | 29.408 | 196.298 | yes |
| moving_congestion | opportunistic | 52.222 | 257.526 | **no** |

All commit, expansion, search-memory, topology-cache and controller-service
assertions passed. All 100 controllers stayed enabled, with maximum service
delay one tick. Peak retained search records were 95,722 (limit 1,000,000), and
no controller exceeded 256 expansions per tick. Every aggregate non-timing CSV
column—including shots, expansions, replans, completions, completion latency and
cache peaks—matches the previous full run. Raw CSV retains these per-case totals.

Compared with the previous full report, weapons-hold autopilot p95 changed by
+2.2% open, +1.4% obstacles and +1.2% congestion, all below the 5% investigation
threshold. The fresh paired short runs improved all hold cases. Whole-heartbeat
firing p95 also improved versus the previous full run; work was not moved outside
the autopilot timing boundary. The earlier report remains intact.

The residual obstacle gap is **5.675 ms**, and congestion is **2.222 ms** over
target. Final detailed profiling ranks contact/sensor/LOS work above shots:
1,433.8 ms inclusive contact time versus 313.8 ms in the existing shots category
across 60 obstacle ticks. Admission/aim still takes 107.8 ms and cross-object
validation 81.3 ms in that profile. Full-run obstacle navigation p95 is 6.857 ms;
congestion navigation is 0.989 ms. These phase percentiles overlap and cannot be
summed. Further contact/geometry profiling and controller observation cost are
the next measured priorities; passing the correctness gates does not close the
remaining timing gap.

## Machine and retained evidence

Measured on the development desktop: Intel Core i7-10875H (8 cores / 16 threads),
32 GiB RAM, Linux 7.1.5-76070105-generic, rustc 1.98.1 / LLVM 22.1.8. Release build:
`CARGO_BUILD_JOBS=4 cargo build --release --bin autopilot-bench`, using Cargo's
default release optimization settings and no RUSTFLAGS override. This desktop is
not an isolated benchmark host; raw measurements and environment details are
retained rather than assuming earlier and later timing conditions are identical.

Final executable SHA-256:
`70e657d71799096de5170558a4560394ec041e414f87e1de4757aeee6a3e1905`.

[Raw artifacts](autopilot-combat-artifacts/README.md) include the
[full CSV](autopilot-combat-artifacts/full.csv),
[paired baseline](autopilot-combat-artifacts/baseline-short.csv),
[paired candidate](autopilot-combat-artifacts/final-short.csv),
[trace comparison](autopilot-combat-artifacts/trace-comparison.json),
[combat attribution](autopilot-combat-artifacts/attribution.json),
[GDB samples](autopilot-combat-artifacts/gdb-stacks.txt),
[executable hashes](autopilot-combat-artifacts/executables.sha256),
[machine/build details](autopilot-combat-artifacts/machine.json),
[checks](autopilot-combat-artifacts/checks.json), and
[acceptance assertions](autopilot-combat-artifacts/acceptance.json).
The preserved baseline source archive, initial empty diff, final working patch,
executables, complete logs and JSONL traces remain under
`/tmp/autopilot-combat-cpu`.

