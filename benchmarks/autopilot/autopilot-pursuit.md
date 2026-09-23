# Predictive interception and pursuit encounters

This follow-up adds observation-derived, bounded interception for Attack and Attack-move. It does not expose target speed, infer hidden movement, or extend contact memory. Runtime navigation aims can lead a visible contact; actual firing, arc alignment and settling still use the observed target. Attack-move retains its six-hex leash.

## Policy

The transient tracker keeps at most six distinct simulation-second samples, using the last four seconds to estimate velocity. It requires three samples spanning two seconds. A change greater than 60 degrees between nonzero observed displacements discards confidence immediately. Quantized sightings can therefore produce intermittent estimates; these are not exact velocity measurements.

The planner checks horizons 1–5 seconds using own admitted speed, the configured speed percentage and map movement rate. Lead is capped at three hexes. It refreshes at three-second intervals and rejects out-of-map or out-of-leash aims. An unreachable predicted region falls back to the observed target before the existing engagement fallback. Existing congestion retry accounting remains authoritative.

Tracking survives normal route replacement, but is discarded on contact loss, lifecycle invalidation, order/target change, terrain/mobility invalidation, restart, or failed commit. It is not serialized. There is no Lua API or production setting. `--direct-pursuit` is an isolated-harness comparison input.

## Reproduction

```sh
cargo build --release --bin autopilot-encounters --bin autopilot-bench
# Each policy runs eight scenarios × four chassis × three seeds × two fire modes.
target/release/autopilot-encounters --suite pursuit --ticks 900 --seeds 3 --direct-pursuit --trace /tmp/direct.jsonl > /tmp/direct.json
target/release/autopilot-encounters --suite pursuit --ticks 900 --seeds 3 --trace /tmp/predict.jsonl > /tmp/predict.json
python3 tools/compare_autopilot_pursuit.py /tmp/direct.json /tmp/predict.json
# Run the candidate again and compare trace files byte for byte.
# The six-case guard is unchanged; the extra workload is explicitly selected.
target/release/autopilot-bench --warmup 35 --ticks 60 --repetitions 3
target/release/autopilot-bench --scenario moving_pursuit --warmup 35 --ticks 60 --repetitions 3
```

All worlds are isolated fixtures. Correctness matrices may use a tmpfs TMPDIR while retaining actual SQLite transactions. CPU acceptance uses the normal filesystem, with no competing builds/tests, tracing or detailed attribution.

The brief-occlusion control uses a standard Jenner target. It waits through tick 199, then follows a validated six-waypoint patrol around one pillar. The expiry control uses a 25% speed observer and a target that drives around the wall end before breaking line of sight. These definitions are frozen before the full paired matrices. The moving-pursuit CPU workload keeps 100 controllers enabled: alternating units pursue legally acquired opponents while those opponents execute ordinary movement orders. Setup uses at most 16 ordinary acquisition refreshes; contacts are never injected.

## Metric interpretation

`first_geometry` means visible, in-range terrain geometry; `first_arc` additionally requires an available weapon to bear. `first_ready` adds estimated readiness and heat admission, or an actual committed shot. It is not a speculative full firing transaction and consumes no dice. Null milestones stay null. Contact episodes describe uninterrupted visibility and include reacquisition-to-opportunity information through their start and first-ready times.

Paired excess travel compares distance to each policy's first opportunity only when both reach it. It is not an optimal interception distance. The stationary route reference is a shortest adjacent-hex route to a terrain-visible weapon-range region; continuous driving and turning need not follow hex centers. Stationary detour distance is diagnostic, not a claim about optimal continuous driving time.

Weapons-hold attack-move does not divert: its control measures ordinary destination travel. Its opportunistic-fire partner tests interception and destination resumption. Firing cases may terminate through destruction and are reported with their actual observation windows rather than compared as equal-duration movement experiments.

## Validation results

**Behavioral acceptance failed. This candidate is not ready to claim improved interception.** The frozen 192-run direct and predicted matrices produced:

| Gate | Measured result |
| --- | --- |
| Lateral median first opportunity | 388 → 388 ticks; **0% gain**, fails the required 10% |
| Individual first-opportunity delay | No weapons-hold regression above five ticks |
| Aggregate hold readiness fraction | 48.757% → 49.095%; passes the two-point tolerance |
| Median paired additional approach distance | 0.0 hexes; passes |
| New sustained blocking | **Nine lateral vehicle cases end `Stuck` at tick 900**; direct runs reach the window end |
| Stationary settling | All 12 hold cases pass the existing limits |
| Short occlusions | Every hold case has at least 20 short reacquisitions |
| Contact expiry | All 24 hold/fire cases fail with `ContactLost` |
| Attack-move destination | All 24 hold/fire cases complete |

The aggregate readiness fractions use each run’s actual visible observation window. Different blocking times change exposure, so the small aggregate increase is not evidence of a general combat improvement.

There are no unmatched hold approach milestones outside the explicitly excluded expiry/destination controls. Destruction remains a separate terminal event in firing runs: the candidate records seven target destructions, eleven `Stuck` outcomes, twelve expiries, twelve completions and 54 observation-window ends. These are not interchangeable successes.

Direct lateral Mechs already become `Stuck` at tick 584; prediction delays that to tick 784 without eliminating it. The three vehicle chassis retain first opportunity at tick 388 but prediction reaches the recovery limit at tick 900. This is a real sustained-pursuit regression even though approach timing passes its individual tolerance. Existing recovery accounting accumulates unsuccessful recoveries over a long order. Further work must establish how prediction-induced goal changes interact with that accounting; this report does not assert that changing the limit is a fix.

Lead is actually used before the first lateral opportunity (27 Mech decision ticks, 84 vehicle ticks for seed 1), so the zero improvement is not explained by prediction being disabled. Quantized sightings and a short bounded lead can change later routes without improving the first firing milestone. That is an interpretation to investigate, not proof of the remaining cause. Fixtures and acceptance thresholds were not relaxed to obtain a pass.

`just checks` passed with 3,061 tests passed, two ignored and no failures. Nineteen Python comparator/attribution tests passed, including a check that an earlier opportunity cannot hide a later blocking regression. Candidate replay passed for **199,580 committed tick records**, across the pursuit, existing, adversarial and clearance matrices. Each complete trace file is byte-identical to its replay, including state/dice/notices digests and outcomes.

The preserved-reference comparison passes the existing behavioral gates for 120 movement and 108 adversarial cases. All 120 movement traces, 36 clearance traces and 72 adversarial traces remain byte-identical. The remaining 36 adversarial cases are `crossing`, `duel` and `occluded` (12 each), where visible target motion can change predictive navigation. Their complete candidate replays are identical, and first-shot/arc/blocking checks retain the prior tolerances. Detailed changed participant outcomes are retained in the artifact report.

The sequential CPU guard uses the preserved executable and candidate, followed by the added 100-controller moving-target workload. The initial paired results are below; timings are milliseconds. No builds, tests or encounter jobs ran concurrently.


| Case | Autopilot p95 before → after | Heartbeat p95 before → after | Candidate guard |
| --- | ---: | ---: | --- |
| open / hold | 44.366 → 46.655 | 210.878 → 219.910 | PASS (50 ms) |
| open / fire | 86.486 → 83.229 | 277.141 → 278.173 | **FAIL** (75 ms) |
| obstacles / hold | 49.937 → 52.229 | 228.298 → 235.755 | **FAIL** (50 ms) |
| obstacles / fire | 80.054 → 82.215 | 284.488 → 299.958 | **FAIL** (75 ms) |
| moving_congestion / hold | 43.591 → 45.275 | 214.604 → 216.276 | PASS (50 ms) |
| moving_congestion / fire | 74.191 → 69.272 | 276.357 → 270.128 | PASS (75 ms) |


The 100-controller moving-target workload passed its absolute guards: **41.084 ms hold / 67.514 ms fire autopilot p95**, with whole-heartbeat p95 **230.146 / 256.322 ms**. It retained 100 enabled controllers, a maximum service delay of one tick, at most 256 expansions per controller and peak retained records of 19,206/19,207. These results do not waive the standard six-case failures.

Open/hold's >5% increase reproduced on the second paired run: **43.570 → 46.069 ms (+5.7%)**. Autopilot p50 increased 37.345 → 38.974 ms, and heartbeat p95 increased 210.980 → 216.642 ms. Observation p95 rose 26.126 → 27.237 ms, movement 18.218 → 18.909 ms, and combat 26.733 → 27.766 ms. Both runs had zero search expansions, replans, shots and renewals in the measured interval. This points away from extra A* work as the explanation, but the phase percentiles do not identify a causal source or add up to an overall percentile. The repeatable hold regression remains unresolved; it is not attributed to SQLite or dismissed as noise.

The obstacles/hold repeat remained above its absolute guard: reference **51.334 ms**, candidate **51.626 ms** (+0.6%); heartbeat p95 **222.661 → 239.154 ms**. The original and repeat failures remain in the artifacts. The preserved reference also exceeded some absolute guards, so these measurements do not isolate prediction as the sole cause of the timing failures.

## Episode attribution and acceptance

Each encounter has one measured pursuit participant; the scripted opponent is a fixture driver, with travel and destruction recorded separately. Contact episodes contain geometry, arcs, readiness, range and travel measurements. `tools/summarize_autopilot_pursuit_episodes.py` derives reversal, replan, prediction-use and fallback counts from successive committed trace totals. It reports work outside contact separately, checks consecutive ticks, and requires episode totals plus outside-contact totals to reconcile exactly with the run summary. It also reports contact-gap duration and reacquisition-to-opportunity delay. The augmented candidate and replay reports must be byte-identical.

```sh
python3 tools/summarize_autopilot_pursuit_episodes.py /tmp/predict.json /tmp/predict.jsonl > /tmp/predict-episodes.json
```

The failed improvement, blocking and CPU gates above mean the approved plan is **not accepted**. This implementation is a measured candidate, not evidence that interception is better or ready for release.

