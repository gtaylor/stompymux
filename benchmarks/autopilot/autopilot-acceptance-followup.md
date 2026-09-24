# Pursuit recovery and acceptance follow-up

This follow-up keeps the existing production predictor. None of the three frozen
experimental policies passed every pursuit gate. The ten-percent lateral
improvement requirement remains unmet; the fixtures and acceptance limits were
not weakened.

## Behavior

Navigation recovery now has transient retry accounting separate from standing.
Actual forward progress or settling replenishes navigation retries. Changed goals
and stale search origins do not. Turning and braking receive at most ten grace
ticks without travel, preventing endlessly changing headings from hiding a stall.
Explicit Attack orders may align torso/turret while weapons are held, without
firing or consuming ammunition, heat, or dice. Attack-move with held weapons
continues to travel without combat diversion.

The harness exposes control/A/B/C policy selection only for isolated comparisons.
A/B/C use timestamped regression over visible sightings with respectively
16/32/32-second windows, 30/60/120-second maximum horizons and 3/6/8-hex lead caps.
Production retains the original estimator. Trace evidence includes visible samples,
velocity, selected horizon, navigation aim and recovery progress.

## Frozen screening

The second screen includes the recovery and held-fire alignment fixes in **both**
the direct and predictive executions. Each policy ran all eight scenarios across
four chassis, seed one, weapons held, up to 900 ticks.

| Policy | Lateral median improvement | Decision |
|---|---:|---|
| A | 2.03% | Below 10% gate |
| B | 7.77% | Below gate; reversal/contact and readiness regressions |
| C | 4.73% | Below gate; individual encounter regressions |

The earlier screen's apparent 25% improvement for A disappeared when held-fire
alignment was fixed in the direct control. It is not evidence of predictive
interception improvement. Both rounds are retained under
[acceptance-followup](acceptance-followup/).

## CPU investigation

Named optional pursuit attribution separates plan cloning, estimation and plan
publication. Existing seven diagnostic categories remain intact. Disabled
attribution reads no clock. Contact observation remains the dominant measured
cost: 1.22–1.37 seconds inclusive across 60 measured heartbeats in the detailed
profiles, compared with 3–18 milliseconds for plan cloning/publication. The post-change
obstacle/firing profile records 1.31 seconds in contacts versus 1.37 seconds
before. These inclusive samples are diagnostic, not acceptance timings. The
retained GDB stack sample is qualitative; a compile may overlap its tail and
it is not used for timing claims.

Scanner projections previously eagerly formatted battlefield labels even for
geometry and sensor queries that never use them. The CPU candidate borrows the
label override and formats only when presentation requests it, using the same
slot/override inputs and formatting rules. No visibility, admission or validation
check is skipped. Gameplay trace comparisons and isolated timing support retaining this optimization.

## Reproduction

Machine, original revision/compiler and executable hashes are in
[provenance.json](acceptance-followup/provenance.json). Reference executables and
large scratch traces are under `/tmp/autopilot-acceptance/`; these are local
artifacts and are not assumed permanent. Timing uses 100 controllers, 100×100 maps,
35 warmup and 60 measured ticks, three repetitions, tracing and attribution off.
Firing/hold p95 limits remain 75/50 ms. Correctness matrices may run concurrently;
CPU acceptance runs execute sequentially after all builds/tests/encounters finish.

## Correctness results

`just checks` passed, including formatting, Lua contract/doc checks and 3,067
passing Rust tests (two ignored). All 20 Python comparator tests passed.
The pre-optimization, optimized and candidate replay pursuit traces match
byte for byte across 137,030 committed records in 192 cases. Per-contact episode
metrics reconcile with the committed traces.

The 120-case movement comparator passes; 108 cases retain identical historical
traces, with changes confined to the twelve moving-target cases. Of 108 historical
adversarial cases, 48 remain byte-identical; changed scenarios are crossing, duel,
occluded, bottleneck and pursuers. The adversarial comparator reports twelve
new `Stuck` outcomes for `pursuer_1`. The alternating-clearance comparator reports
six new `Stuck` Mech blockers, while focal units still complete. These are **failed
regression gates**, not approved exceptions. The finite steering grace exposes
stationary behavior previously kept alive by control changes; further recovery
work is needed before this behavior change is acceptance-ready.

All 21 before/after or replay trace-file comparisons passed. Candidate replay
covers 200,390 committed records, including all 264 existing/adversarial/clearance
cases. The full 192-case pursuit comparison misses only the lateral-improvement
gate: median gain is 0%; readiness fractions are 0.736739 direct and 0.736599
predictive. No experimental policy is promoted.

## Paired CPU results

These sequential runs compare identical behavior before/after lazy label
formatting, not the original pre-recovery revision. Both executables use the
same release compiler/profile and workload. All deterministic resource/service
assertions pass. No hold case regresses by more than 5%.

| Case | AP p50 before → after (ms) | AP p95 before → after (ms) | Heartbeat p95 before → after (ms) |
|---|---:|---:|---:|
| Open hold | 34.084 → 29.837 | 36.761 → 32.172 | 167.830 → 159.997 |
| Open firing | 40.449 → 36.526 | 62.504 → 61.467 | 215.024 → 207.577 |
| Obstacles hold | 38.653 → 34.135 | 41.511 → 37.042 | 183.309 → 172.055 |
| Obstacles firing | 47.584 → 43.850 | 64.674 → 61.379 | 232.029 → 225.696 |
| Congestion hold | 34.454 → 30.188 | 36.238 → 31.380 | 167.778 → 159.292 |
| Congestion firing | 41.987 → 37.968 | 58.520 → 54.689 | 217.680 → 211.651 |

All six pass the unchanged 75 ms firing / 50 ms hold guards. Autopilot p95 savings
are 4.47–4.86 ms for hold and 1.04–3.83 ms for firing; whole-heartbeat p95 falls
6.03–11.25 ms. These are paired measurements on one development machine, not a
portable latency guarantee. Earlier reports remain intact. The fresh original
reference also passed its guards, so historical slower runs are not attributed
to this optimization.

See [raw baseline CSV](acceptance-followup/cpu-before.csv),
[raw candidate CSV](acceptance-followup/cpu-final.csv),
[comparisons](acceptance-followup/cpu-comparison.json),
[trace hashes](acceptance-followup/trace-comparisons.json) and
[executable hashes](acceptance-followup/executable-hashes.json).

The separate 100-controller moving-target workload also passes. Autopilot p95
falls from 33.029 to 29.347 ms with weapons held and 53.617 to 50.838 ms firing;
whole-heartbeat p95 falls from 186.171 to 179.447 ms and 203.849 to 197.070 ms.
Search expansions, replans, shots and service counts match. Raw
[before](acceptance-followup/moving-before.csv) and
[after](acceptance-followup/moving-final.csv) CSVs retain all metrics.

## Remaining acceptance work

The CPU optimization is equivalent and improves the measured workloads, but the
behavior package is **not acceptance-ready**. Resolve the eighteen legacy
recovery blocking regressions and achieve the frozen interception improvement
gate before claiming pursuit acceptance. This report does not waive any gate.
No Lua API, persistent schema, fixture difficulty, production predictor policy,
visibility rule or contact-expiry boundary was changed.
