# Pursuit acceptance readiness fixes

This report follows the failed gates in
[the acceptance follow-up](autopilot-acceptance-followup.md). The earlier report
and evidence remain unchanged. Fixtures, comparator limits and CPU guards are
unchanged.

## Root causes and changes

An occupied waypoint caused a permanent yield to a known lower-ID unit, even
when that occupant had completed its order and ordinary traversal permitted
entry. Yielding now gives five simulation seconds of courtesy at that waypoint,
then attempts ordinary admitted movement. The deadline survives replanning;
changing waypoints starts a new bounded wait. Crowded terrain remains subject
to existing admission and search rules. No hidden occupancy or global reservation
is added. Checkpoint equality includes this transient state.

Combat alignment could mistake a unit stopped to turn along its route for a
settled unit, and replace its navigation heading. While a route or search owns
movement, combat may align mounts but cannot replace the hull heading. Settled
units retain ordinary hull alignment. Navigation retry budgets and finite turning
grace are not increased.

Short histories often contain only one transition for a slow target. Offset hex
centers make these isolated transitions look like alternating diagonal motion,
and an interval between transitions can look stationary. The selected G estimator
uses timestamped regression over at most 65 samples in 64 seconds, evaluating at
most 120 integer horizons with an eight-hex lead cap. These are hard bounds.
It retains the existing three-second reconsideration cadence, immediate observed
reversal reset, contact-loss reset, actual-target settling, and attack-move leash.
It reads no scripted routes or hidden motion. Thirty-second last-sighting expiry
is unchanged. The policy selector remains an isolated harness option, not Lua or
a persisted setting.

## Screening

All eight scenarios and four chassis were screened with seed one, weapons held,
900-tick maximum. D/E retained a 16-second window with longer horizons; F used the
64-second window with a six-hex cap; G used the same window with an eight-hex cap.
Only G passes every screen gate. Its lateral median is 238 ticks versus 296 direct
(19.59% faster); the Mech improves from 115 to 103 ticks. Readiness fraction rises
from 0.736796 to 0.745667; median paired additional approach distance is -0.15625
hex. A separate approach-only prediction experiment is retained but not selected;
G passed without changing the existing preferred-range settling rule.

## Full encounter acceptance

Every comparator passes against the unchanged gates: 192 pursuit cases,
120 movement cases, 108 adversarial cases and 36 clearance cases. The eighteen
blocking regressions are removed across all three seeds. The full pursuit
comparison confirms 19.59% median lateral improvement, readiness fractions
0.736739 direct versus 0.745635 predictive, and median paired additional approach
distance -0.15625 hex. No individual feasible case regresses by more than five
ticks. Stationary settling, contact expiry, reacquisition and attack-move
completion checks pass.

All thirteen candidate/replay trace files match byte for byte, covering 200,420
committed records (137,060 pursuit plus 63,360 regression). Episode metrics
reconcile with committed pursuit traces. See the retained
[comparisons](readiness-fix/pursuit-comparison.json) and
[trace hashes](readiness-fix/trace-comparisons.json).

The historical trace audit compares common fields, excluding the newly added
navigation diagnostics. It retains identical records in 108/120 movement cases
(changes only in `moving`) and 48/108 adversarial cases. Changed adversarial
families are crossing, duel, occlusion, bottleneck, pursuers and Mech passage;
traffic/clearance changes reflect bounded yielding and recovery accounting.
All outcome/timing/coverage comparators pass. Per-case changed-record counts and
first changed ticks are retained in
[the trace audit](readiness-fix/behavior-trace-differences.json).

## Repository checks

`just checks` passed: 3,069 Rust tests passed, two were ignored, and Rust/Lua
formatting plus generated Lua types/docs checks passed. All 20 Python comparator
tests passed. New focused coverage verifies bounded courtesy waits and checkpoint
restoration, preservation of navigation heading during combat alignment, bounded
estimator history/lead for every policy, timestamp-origin invariance, slow
quantized motion and immediate reversal reset.

## Evidence and protocol

[Retained artifacts](readiness-fix/) include screening summaries, executable hashes,
source patch and runner scripts. Large local traces and preserved executables live
under `/tmp/autopilot-ready/`. The development machine and compiler are the same
as recorded in [prior provenance](acceptance-followup/provenance.json).

Full validation uses 192 pursuit cases per policy/replay, 120 movement,
108 adversarial and 36 clearance cases. CPU timing runs sequentially after all
correctness/build work, with 100 controllers on 100×100 maps, 35 warmup and 60
measured ticks, three repetitions, tracing/attribution disabled. The guards remain
75 ms firing / 50 ms hold. A separate moving-target pair uses the same protocol.

## CPU guard investigation

The first fresh unpinned pair exceeded the firing guard in both the preserved
reference and behavior-fixed build: open firing was 83.244/83.335 ms, obstacles
80.937/77.740 ms. Affinity-only repeats did not solve the obstacle case; their
results are retained and are not used as final acceptance. No guard was raised.

Three read-only geometry changes address the remaining work: sensor/contact
queries reuse the range already computed for LOS; distances below both possible
LOS ceilings or above both skip irrelevant radar hardware projection; and unit
range reuses its already-validated horizontal range when calculating bearing,
with the exact zero-height spatial-distance shortcut. Public checked bearing
calls retain validation. No geometry survives a world mutation and no weapon
admission or transaction boundary changes.

Focused unpinned checks reached 73.813 ms obstacle firing after the LOS changes
and 72.061 ms open firing after range reuse. The final optimized build passes all
six CPU guards, all behavior comparators, and `just checks` (3,069 passing Rust
tests, two ignored). Its 200,420 committed records match both the pre-optimization
behavior build and an independent optimized replay byte for byte: 26 file
comparisons. See [final equivalence evidence](readiness-fix/optimized-trace-comparisons.json)
and [final pursuit gates](readiness-fix/optimized-pursuit-comparison.json).

The final sequential unpinned pair compares the same accepted behavior before
and after geometry reuse. All measurements below are milliseconds. No hold case
regresses; whole-heartbeat timings improve alongside autopilot timings.

| Scenario | Fire | Autopilot p50 before → after | Autopilot p95 before → after | Heartbeat p95 before → after |
| --- | --- | --- | --- | --- |
| open | hold | 29.450 → 25.962 | 31.736 → 28.170 | 158.925 → 147.831 |
| open | enabled | 36.210 → 31.762 | 60.099 → 56.392 | 205.941 → 193.951 |
| obstacles | hold | 34.016 → 30.349 | 36.508 → 32.625 | 171.407 → 161.173 |
| obstacles | enabled | 44.309 → 39.381 | 61.312 → 56.384 | 224.161 → 210.675 |
| moving_congestion | hold | 30.092 → 26.435 | 31.679 → 27.738 | 158.616 → 147.721 |
| moving_congestion | enabled | 37.979 → 33.490 | 53.462 → 50.259 | 208.889 → 196.731 |

Firing autopilot p95 improves 6.0–8.0%; hold improves 10.6–12.4%. Search work,
shots, enabled-controller counts and service-delay counters match the paired
reference. Every case services all 100 controllers within one tick; per-controller
search expansion maxima remain at or below 256, with peak search records 89,770.
Raw [reference](readiness-fix/cpu-geometry-reference.csv) and
[candidate](readiness-fix/cpu-optimized.csv) include persistence and phase timings.

The reference itself ran faster in this final pair than in the earlier slow runs.
The machine reported about 4.2 GHz during final timing versus about 3.4 GHz earlier,
under the same powersave governor with AC connected. This is an observed source
of variability, not a controlled frequency experiment; do not attribute the whole
historical difference to the optimization. No power settings or CPU affinity were
changed for acceptance. Earlier failures, affinity experiments and incremental
measurements remain retained. These results establish the documented short CPU
guard, not a universal latency guarantee or a new 18,000-heartbeat soak result.

## Moving-target CPU workload

The separate 100-controller moving-target pair also passes both guards under the
same 35/60/three-repetition protocol. Hold autopilot p50/p95 changes from
26.896/30.388 to 24.588/27.771 ms; firing changes from 30.753/51.376 to
28.433/48.839 ms. Whole-heartbeat p95 changes from 191.187 to 177.971 ms hold,
and 201.085 to 189.534 ms firing. Search expansions, replans and shots are
identical between builds; all controllers receive service within one tick and no
orders block in this workload. Peak search records are 29,308, and the terrain
trace cache reaches its existing one-million-cell cap without exceeding it.

See [moving reference](readiness-fix/moving-geometry-reference.csv) and
[moving candidate](readiness-fix/moving-optimized.csv). This workload measures
ongoing pursuit/replanning overhead; it does not replace the 900-tick encounter
completion gates. No acceptance blocker remains in the tested matrices.
