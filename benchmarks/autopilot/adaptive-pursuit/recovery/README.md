# Sustained engagement recovery

Measurement verdict: full validation completed; **acceptance failed on moving-target
hold CPU**. Earlier failed measurements and focused screens are unchanged.

Deployment decision (2026-09-24): at the user's explicit request, Adaptive is now
the production default despite the recorded CPU regression. Unqualified benchmark
and encounter runs also select Adaptive. G remains available only as an explicit
comparison policy; acceptance reference runs continue to request G. This default
selection change does not turn the measured CPU failure into a pass, and the
captured executable hashes below describe the pre-promotion measured candidate.

Subsequent cleanup: at the user's request, G has been removed from the source and
CLI selectors. New acceptance runs compare explicit Adaptive captures. The
measurements, reviews, and failed verdict below retain their historical meaning;
the G executables and tools used for those runs remain in ignored artifact storage.

The pre-change source archive, working-tree diff, executable copies, and hashes are
in ignored storage at `target/autopilot-acceptance/recovery-reference/`. This is
distinct from the sealed `adaptive-validation3` candidate and its failed verdict.

## Candidate changes

- CPU benchmarks select G, Adaptive, or direct pursuit explicitly. Each heartbeat
  verifies the requested policy; a separate metadata file preserves CSV meanings.
  Acceptance selects Adaptive explicitly and checks encounter/CPU policy identity.
- Adaptive distinguishes positional geometry from actual mount bearing. Useful
  geometry in the preferred band suspends scoring, preserving the three-second
  reconsideration cadence. Loss permits reassessment at the next boundary.
- Cached and new lead regions are ineligible when already occupied without useful
  observed positional geometry. Rejection never counts as physical progress.
- Execution and candidate scoring share direction choice, braking before reversal,
  and the reverse speed cap. Reliable observed radial velocity can prefer forward
  closure when reversing cannot sustain range. No extra forecast pool is added.
- Named opt-in pursuit attribution separates fitting, assessment, scoring,
  projection work, engagement transitions, and replacement/congestion searches.
  The Rust timeline reports geometry, arcs, readiness, and actual shots separately.

The fitting windows, retrospective confidence thresholds, three candidate limit,
16-proposal limit, search budgets, fixtures, and behavioral/performance gates are
unchanged. No semantics-preserving CPU optimization is accepted without paired
measurement and replay evidence. The moving CPU workload is nearby-target pursuit,
not a distant-interception stress benchmark.

## First screen

`screen1.json` contains seven scenarios, four chassis, one frozen seed, weapons
hold, 900 ticks. Vehicle fast-crossing readiness improves from the failed full-run
candidate's 349 ticks to 587 (direct: 572), with first opportunity still at tick
314 (direct: 329). Slow and opposite crossings no longer block. Lateral remains
248 ticks, stop/start 216, gradual turns 239, and damaged pursuit 354 for vehicles.
These are focused results, not full acceptance.

The first detailed 100-controller Adaptive run aborted at the existing 16-check
geometry assertion: the new observed-position check had not been subtracted from
the goal-filtering allowance. The next candidate charges both against the same
unchanged budget. No timing verdict is derived from the aborted run.

## Measured equipment reuse

Screen 2's detailed moving-target hold run completed all resource assertions.
It measured 59,123 equipment resolutions for Adaptive versus 7,642 for G over
60 measured ticks. Fitting (5.6 ms total) and plan copying/publication (29.2 ms
combined total) were substantially smaller than candidate scoring (113.9 ms).
The retained optimization scopes immutable own-equipment reuse across Adaptive's
read-only planning block, ending before admitted world mutations.

Screen 3 reduces equipment resolutions to 8,271. Both versions perform exactly
1,783 scores, 24,837 own-motion projections, and 648 replans. The separately
captured 95 committed tick records match byte for byte, including the gameplay
and ordered-notice digests (`loadout-traces-sha256.txt`). No history-storage or
regression-fitting optimization was retained.

Sequential, uninstrumented one-repetition runs (35 warmup, 60 measured ticks)
reduced autopilot p95 from 34.245 to 29.681 ms and p50 from 25.661 to 25.122 ms.
Whole-heartbeat p95 increased from 172.397 to 175.243 ms; its p50 decreased from
152.725 to 151.494 ms. These short paired results support the optimization but
are not a substitute for the full repeated CPU guards. Raw timing CSVs and named
attribution are retained alongside this report.

## Acceptance candidate

Final review separated cached positional eligibility from preferred-band membership
so a previously admitted fallback band still permits settling. A focused regression
uses an impossible preferred band and verifies the usable fallback accumulates no
stuck ticks. The geometry-budget regression reproduces the original assertion in
one isolated 100-controller tick. The focused suite passes 97 tests, with one
ignored, in 0.84 seconds after compilation.

The initial full run at `target/autopilot-acceptance/recovery-validation/` was
interrupted during checks to apply the fallback correction; it is not acceptance
evidence. Run `recovery-validation2` passed checks but was interrupted before
completing encounters to strengthen historical-policy provenance validation and
increase correctness-only concurrency. The final run is
`target/autopilot-acceptance/recovery-validation3/`.
Its candidate and reference execution policies are explicit, including the
established movement and adversarial harnesses. Historical reference execution uses
the captured policy-G source and executable provenance.

The final run passes 3,133 tests (two ignored) across 22 suites, including
`just checks`; all 27 acceptance-tool comparator tests pass. Correctness matrices
run with eight workers. CPU timing runs sequentially after those workers finish.
These checks alone do not establish behavioral or performance acceptance.

The 192-case established behavioral comparison passes: lateral improvement is
16.216%, with readiness 73.674% direct versus 74.417% Adaptive. The 144-case
expanded comparison also passes: readiness is 75.565% direct versus 76.097%
Adaptive, recovering the earlier failed candidate's 70.293%. Median paired
additional approach distance is zero. No fixture, gate, or tolerance changed.
The isolated opposite-crossing comparison reports a roughly 7e-15-hex distance
increase; the unchanged full expanded-matrix distance gate passes.

Reference gameplay is identical for all 120 movement, 108 adversarial, and 36
clearance cases. Among pursuit groups, distant, expiry, Attack-move, reversals,
short occlusions, and slow crossing have identical gameplay digest sequences;
their new policy/decision diagnostics differ. The other eight groups change
gameplay. Per-case milestones and diagnostic attribution are in
`reviewed-milestones.json`; terminal outcomes remain unchanged. Stop/start vehicle
opportunities are three ticks later and retreating Mech opportunities two ticks
later than direct pursuit, within the unchanged five-tick limit. Against G,
lateral armed Mech seed 1 fires 89 rather than 96 shots, and damaged-pursuit armed
Mech seed 5 fires 88 rather than 91. These differences are retained explicitly,
not treated as diagnostic-only changes.

## Final verdict

All behavioral and replay gates pass. Candidate replay matches **329,927 committed
tick records** byte for byte, including state/dice and ordered notices, and all
19 replay summaries match. The 63,360 movement/adversarial/clearance reference
records are byte-identical. Exact reference hashes and per-case reviews are in
`reviewed-changes.json`; the sealed runner's original reference-review failures
remain intact because that file was supplied after measurement. The preserved
finalizer validates those reviews, then correctly refuses to waive the CPU failure
(`Non-reference acceptance gate failed; review cannot waive it`).

Both CPU workloads use 100 controllers, 35 warmup ticks, 60 measured ticks, and
three repetitions, sequentially with tracing and detailed attribution disabled.
All resource assertions and absolute 50 ms hold / 75 ms firing guards pass.
The six standard cases also pass the relative hold-regression guard.

| Scenario | Fire | G AP p95 ms | Adaptive AP p95 ms | G heartbeat p95 ms | Adaptive heartbeat p95 ms |
|---|---|---:|---:|---:|---:|
| Open | Hold | 27.495 | 27.637 | 154.699 | 153.625 |
| Open | Fire | 50.897 | 52.705 | 194.670 | 195.733 |
| Obstacles | Hold | 31.952 | 31.537 | 167.349 | 164.653 |
| Obstacles | Fire | 53.810 | 54.320 | 206.033 | 214.660 |
| Congestion | Hold | 26.632 | 27.204 | 152.470 | 154.396 |
| Congestion | Fire | 47.686 | 47.963 | 196.190 | 200.225 |
| Nearby moving targets | Hold | 25.957 | 29.800 | 170.538 | 176.787 |
| Nearby moving targets | Fire | 47.460 | 52.949 | 185.571 | 195.936 |

The remaining failure is **14.805% moving-target hold regression**, above the
unchanged 5% threshold. Its autopilot p50 is 23.483 -> 25.311 ms; whole-heartbeat
p50 is 146.653 -> 151.205 ms. This is not a timing pass merely because the absolute
guard passes. The earlier 33.32 -> 37.00 ms result is from a different run; the
current paired reference is authoritative for this verdict.

## Remaining measured work

Separate same-executable, explicit G/Adaptive attribution runs after acceptance
are in `final-g-attribution.json` and `final-adaptive-attribution.json`. These
instrumented one-repetition runs are diagnostic only. Over 60 measured ticks:

- Adaptive estimation takes 61.56 ms total versus 4.99 ms for G (about 0.94 ms
  additional per tick). Its inclusive choose scope is 47.68 ms, including 36.86 ms
  candidate scoring and 18.18 ms of own-motion projections. Inclusive scopes must
  not be summed. There are 1,783 scores and 24,837 projections.
- Adaptive starts 648 searches versus 437: 359 replacements and 289 ordinary
  route searches. The full three-repetition run has 1,944 versus 1,311 replans,
  despite fewer expansions (540,942 versus 551,106). More restarts alone therefore
  do not prove an expansion-cost explanation.
- Contact calls increase from 64,450 to 73,300, and geometry/sensor queries from
  318,257 to 321,392. Movement-phase p95 is 23.701 versus 19.861 ms in the
  uninstrumented full run. These costs and the estimate/goal-update work remain
  the measured area for further optimization; this profile does not completely
  attribute the p95 difference.
- Equipment resolutions are 8,271 versus 7,642, confirming the retained reuse
  removed the previous 59,123-resolution amplification. Plan clone/publication
  together differ by only about 3.60 ms total; no history-storage refactor was
  justified by this evidence.

Raw acceptance CSVs, policy identity, machine samples, candidate executable hashes,
source provenance, replay results, and original failed verdict are retained here.
Large traces, source snapshots, and executables remain in ignored
`target/autopilot-acceptance/recovery-validation3/`. Final diagnostic logs remain
in `target/autopilot-acceptance/recovery-final-profile/`. No further execution-code
change was made during that validation run. The subsequent user-requested promotion
changes default policy selection only. No fixture, budget, threshold, tolerance,
or production API was changed.
