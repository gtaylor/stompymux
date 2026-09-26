# Adaptive pursuit redesign

**Not accepted.** The redesign clears lateral improvement but fails sustained
engagement in the expanded matrix. Production and encounter defaults remain policy
G. The redesign is available only through the existing isolated-harness selector
`--pursuit-policy adaptive`; no game setting or Lua API was added.

The complete measurement below used the preserved candidate with Adaptive as its
default. Subsequent stale-goal and reverse-motion fixes have focused-screen evidence
only, not full acceptance. Restoring G as the default does not turn the failed
adaptive measurement into a passing verdict.
The preserved pre-change candidate is `target/autopilot-acceptance/adaptive-reference/`
(revision `2f69adb2e4e7e77f8589f188bb440be5d49acfb8`). Its encounter executable hashes
match the prior Rust acceptance report. Earlier reference captures remain intact.

The experimental adaptive policy compares stationary motion with 16- and 64-second
constant-velocity fits. The selected revision uses retrospective 6/12/24-second
forecasts over the latest sixteen observed outcomes, requiring the 24-second offset
and at least one shorter offset, three observed transitions, and a 20% improvement
over stationary forecast error. Selection retains the 65-sample, 120-horizon,
eight-hex bounds. Two fits are made per new observation; historical fits are retained
without refitting them against later observations. Assessment performs at most 96
forecast-error comparisons per reconsideration.
At most three candidates receive at most sixteen own-motion proposals each.
The selected score minimizes worst-case regret between continued motion and a stop
at the observed position. Goal changes require three seconds of estimated regret
improvement; an incumbent goal can
replace the half-lead candidate to keep hysteresis within the three-candidate cap.

A single replacement A* frontier may coexist with the previously admitted route.
Only a live-validated prefix is driven while that frontier is pending. This uses
the same search and storage budgets and is transient/checkpointed. No Lua or
persistence surface is changed.

The Rust `autopilot-acceptance timeline TRACE` command reports observed decision
categories and explicitly does not assert causality. Old traces without motion
fields report zero motion samples, not proof of zero braking or waiting.

## Initial screen

The preserved candidate starts vehicle lateral prediction at tick 103 and reaches
its first opportunity at 269; stop/start never predicts and reaches it at 213.
The first adaptive screen produced no predictions: startup hex intervals were
incorrectly treated as an established pause cadence. Vehicle lateral therefore
matched direct pursuit at 296. A deterministic uneven-interval test now requires
three observed transitions before pause cadence is trusted. Fixtures and gates
are unchanged; this is a detector correction, not a passing performance result.

## Scoring and confidence experiments

Screens 2–6 retained the original thresholds and did not clear the lateral gate.
Retaining historical fits corrected truncated-prefix revalidation, and the
moving-target closure estimate corrected a stationary residual assumption.
The region-edge correction removed uncharged travel to a goal center. Nonetheless,
vehicle lateral first opportunity was 299 ticks in screen 6 (direct: 296), with
99 prediction ticks interrupted by confidence loss; stop/start remained 213 and
damaged vehicles improved to 352 from 355.

The next policy revision evaluates the same 6/12/24-second retrospective offsets
over the latest sixteen observed outcomes, rather than just the current hex phase.
It still requires two distinct valid offsets and 20% lower mean hex error than
stationary prediction. This deliberately recorded assessment-window change
addresses confidence oscillation on steady quantized motion; it adds at most 96
error comparisons per reconsideration, no model fits or navigation searches.
The fixture matrices and acceptance thresholds remain frozen.

The fitted-line revision retained both regression intercept and velocity in
historical forecasts and navigation estimates. Screen 8 cleared lateral improvement
(vehicle first opportunity 241 versus direct 296) but regressed stop/start to 227
versus 213. That revision is not accepted. Both attempted full runs were stopped
in checks and remain explicitly incomplete, without performance results.

The next goal selector minimizes worst-case regret across continuing-motion and
stopping-at-the-observed-position estimates. Both use the same bounded own-motion
forecast; stationary-region entry is evaluated before the navigation endpoint.
This is a recorded policy change motivated by the stop/start failure. It adds no
forecast steps or searches. Diagnostics expose each hypothesis's estimated seconds
and the resulting regret; the three-second switching margin now applies to regret.
No hypothetical stop is treated as an observed target action.

Screen 9's regret selector retained lateral 241 and stop/start 227. Screen 10
applied corridor lookahead equally to predictive and direct pursuit: lateral was
241/284 and stop/start 222/205. The stop/start gap worsened, so that steering
extension was removed. The retained corridor behavior remains prediction-only.

The subsequent confidence revision requires the 24-second retrospective offset
along with at least one shorter offset before using a long lead. This records a
more conservative calibration for extrapolation up to 120 seconds; the minimum
two-offset and 20% error-improvement requirements remain. The change applies to
all scenarios and does not use target scripts or scenario identities.

## Candidate selected for full validation

Screen 11 (four scenarios × four chassis, one seed, weapons hold, 450 ticks)
reported vehicle lateral 248 versus direct 296, stop/start 216 versus 213,
gradual turns 239 versus 262, and damaged pursuit 354 versus 355. Mech first
opportunities remained 115, 105, 109, and 131 respectively. This screen satisfies
the individual delay limits and lateral improvement threshold, but is not full
acceptance. The final candidate includes explicit committed confidence-reset
reasons so pause/reversal events are not hidden by the following model decision.

Validation directory: `target/autopilot-acceptance/adaptive-validation3/`.

## Full-matrix failures and follow-up screens

The selected candidate clears lateral improvement (16.22%) and the stop/start,
gradual-turn, and damaged-mobility gates, but fails sustained slow, fast, and
opposite crossing. It is not accepted. Slow-crossing Mechs stop at a retained aim
behind the observed target and reach `Stuck` at tick 193. Opposite-crossing vehicles
also block. Fast-crossing vehicle opportunity count falls to 349/900; traces show
extended reverse pursuit while scoring assumes forward closure.

Screen 12 rejects aims already passed along the observed velocity. At seed 4,
slow crossing now matches direct pursuit (Mech 800 ready ticks, vehicles 710), and
opposite crossing reaches the window end for every chassis. Fast crossing remains
349 ready ticks for vehicles. This isolates the stale-goal fix from the remaining
reverse-motion scoring mismatch. The next bounded revision scores only direct
pursuit while actual own motion is reversing; confidence history remains available
for subsequent forward motion. Neither fixtures nor acceptance gates change.

Screen 13 tests that reverse-motion restriction. Fast-crossing vehicles retain the
314-tick first opportunity, but increase readiness-qualified opportunity ticks only
from 349 to 360, versus direct pursuit's 572. The first interception is earlier
(direct: 329), but subsequent positioning is substantially worse. This is still a
failed redesign: dropping unsupported estimates alone does not recover sustained
engagement after the initial predictive route. The regression is not solved by
clearing the lateral threshold.

The isolated opposite-crossing comparison also trips its distance check at a positive
`7.105427357601002e-15` hex difference; the complete expanded suite's median-distance
gate passes. No tolerance or exception was added. A future
candidate must address the remaining positioning/scoring mismatch and rerun every
gate; the small follow-up screens cannot replace that validation.

## Complete encounter verdict

The measured candidate passes all 192 established pursuit cases, including the
16.22% lateral gain. Established-suite readiness fraction is 73.67% direct versus
74.42% adaptive. The 144 expanded cases fail: twelve hold cases terminate `Stuck`
(three slow-crossing Mechs and nine opposite-crossing vehicles), and aggregate
readiness falls from 75.56% to 70.29%, beyond the two-percentage-point allowance.
Median paired additional approach distance passes at the full-suite level.

All 120 movement, 108 adversarial, and 36 clearance encounters pass and retain
byte-identical reference traces. All nineteen candidate replay comparisons pass:
319,169 committed tick records match byte for byte, including gameplay digests,
ordered notices, and committed outcomes. The measured build passed `just checks`
(3,124 tests passed, two ignored). Follow-up source checks are reported separately.

## CPU protocol and six-case results

Both executables ran sequentially with 100 controllers on 100×100 maps, three
repetitions, 35 warmup ticks and 60 measured ticks. Tracing and detailed attribution
were disabled. Timed SQLite fixtures used system-default temporary storage;
correctness fixtures used isolated `/dev/shm` directories. No build, test, or extra
encounter run overlapped the CPU phase.

| Scenario | Fire | Reference AP p50/p95 ms | Adaptive AP p50/p95 ms | Reference heartbeat p50/p95 ms | Adaptive heartbeat p50/p95 ms |
|---|---|---:|---:|---:|---:|
| Open | Hold | 31.47 / 37.39 | 29.93 / 35.06 | 169.64 / 200.73 | 165.82 / 195.61 |
| Open | Fire | 37.88 / 73.72 | 37.73 / 73.16 | 199.31 / 255.46 | 199.85 / 257.96 |
| Obstacles | Hold | 35.16 / 40.91 | 34.82 / 40.90 | 182.92 / 201.83 | 180.82 / 207.32 |
| Obstacles | Fire | 46.22 / 70.94 | 46.16 / 73.39 | 208.92 / 263.89 | 210.75 / 269.68 |
| Moving congestion | Hold | 31.13 / 34.84 | 30.92 / 36.21 | 177.65 / 194.33 | 179.47 / 197.82 |
| Moving congestion | Fire | 40.23 / 63.53 | 39.70 / 59.77 | 208.41 / 245.79 | 205.11 / 253.05 |

The six-case 75 ms firing / 50 ms hold guards pass. The largest hold p95 increase
is 3.95%, below the 5% investigation threshold. These are timings for the fully
measured failed candidate, not acceptance evidence for the later screened fixes.
The existing moving-target workload starts nearby and must not be interpreted as
a distant-prediction stress benchmark.

| Moving-target workload | Reference AP p50/p95 ms | Adaptive AP p50/p95 ms | Reference heartbeat p50/p95 ms | Adaptive heartbeat p50/p95 ms |
|---|---:|---:|---:|---:|
| Hold | 29.01 / 33.32 | 30.93 / 37.00 | 182.40 / 216.21 | 182.64 / 213.13 |
| Fire | 33.51 / 64.45 | 36.71 / 68.81 | 199.48 / 236.62 | 207.00 / 242.52 |

**The moving workload fails the hold-regression gate:** hold p95 increases 11.04%,
despite remaining below 50 ms. The absolute firing bound also passes, but does not
waive this failure. Investigation of the retained phase counters locates the
increase primarily in movement: movement p95 rises 25.25 → 28.57 ms, navigation
6.21 → 6.97 ms, observation 11.90 → 12.72 ms, and combat 8.37 → 8.73 ms. These
quantiles overlap and cannot be added or interpreted as causal attribution.
Replans rise 1,311 → 2,043 (+55.84%), while expansions rise only 551,106 → 552,129
(+0.19%) and peak search records rise 25,756 → 30,215. Whole-heartbeat hold p95
falls slightly; that does not excuse the autopilot-specific regression. Further
profiling must separate goal churn, model fitting, and retained-route overhead.

## Reference review and retained artifacts

[The per-case review](validation/reference-case-review.json) binds all 336 pursuit
case identities to both trace hashes and ordered gameplay-digest sequence hashes.
138 cases differ only in diagnostics; 198 change gameplay. The digest covers
serialized Btech state, ordered notices, and committed shots. Each changed case
retains before/after milestones, opportunity counts, distance, replans, destruction,
and terminal outcome. Lateral and gradual-turn gains, bounded retreat/stop-start
changes, and unchanged milestones with different movement paths were reviewed
against the frozen gates. Slow/opposite blocking and fast-crossing opportunity
loss remain rejected. There are 24 changed terminal outcomes when both fire modes
are counted, including the twelve hold failures enforced by the comparator.

No review waiver or successful finalization was issued. The original
[machine verdict](validation/report.json) and [readable report](validation/report.md)
remain failed, including unresolved reference-difference gates. The review explains
the differences; it does not approve a behaviorally failing candidate.

Compact CSVs, comparator results, replay counts, timelines and executable/source
manifests are in [validation/](validation/). Large JSONL traces, binaries, source
snapshots, command logs and the sealed artifact hash inventory remain in ignored
`target/autopilot-acceptance/adaptive-validation3/`. Earlier captures and reports
were retained. Screens 11–13 are explicitly partial comparisons, not acceptance.

Machine: Intel Core i7-10875H (8 cores / 16 threads), Linux x86-64, powersave
governor; Rust 1.98.1 / LLVM 22.1.8, release profile, no `RUSTFLAGS` override. The
candidate [manifest](validation/candidate-manifest.json) records compiler, fixture
and source hashes; the preserved reference manifest is alongside this report.

To exercise the final experimental source explicitly (this is not the fully
measured candidate executable):

```sh
cargo run --release --bin autopilot-encounters -- \
  --suite pursuit_extended --pursuit-policy adaptive --ticks 900 \
  --seed-start 4 --seeds 3
```

Final-source hashes are in [final-source-sha256.txt](final-source-sha256.txt).
Its source archive and patch are retained under ignored
`target/autopilot-acceptance/adaptive-review/`. The fully measured candidate's
source snapshot remains in its separate sealed validation directory.

Final source verification: `cargo fmt` and `just checks` passed, including
`cargo test` (3,125 passed, two ignored across 22 test/doc-test suites), Lua type
checks and the generated 562-page Lua reference check. The Rust comparator and
timeline tests passed. [Check summary](final-checks.json) and
[log hashes](final-checks-sha256.txt) bind this result to the final source.
Passing these checks does not change the failed adaptive acceptance verdict.
