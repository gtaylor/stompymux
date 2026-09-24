# Rust acceptance automation and expanded pursuit

The current workflow is documented in [autopilot-acceptance.md](../autopilot-acceptance.md).
All current acceptance tooling is Rust. The `legacy-tools/` directory preserves
the source of historical comparators for earlier reports; no current recipe calls them.

The expanded matrix is frozen in
[pursuit-extended-fixtures.json](../../../design/autopilot/pursuit-extended-fixtures.json):
six scenarios, four chassis, seeds 4–6, and weapons hold/assigned-target fire.
Setup validates distant visible contact and uses ordinary target motion. The
damaged-mobility case applies actual damage at tick 30. Script events are recorded
separately from observed movement and readiness.

Reference executables were preserved before production changes in
`target/autopilot-acceptance/reference/`:

| Executable | SHA-256 |
| --- | --- |
| autopilot-encounters | dafff336a68f670ee2449ac4099d46a1baffe904573904021938124410eda3a7 |
| autopilot-bench | df724fb9d88822adddcb9d190b87a7ef43f8cfccd6c7d72d2c8d7b72b4f72742 |

That capture includes compiler/machine information, the source patch, and a source
snapshot. Large executables and traces remain in ignored `target/` storage.
Earlier benchmark reports remain unchanged.

Validation completed on 2026-09-24. **The candidate is not acceptance-ready.**
The [complete report](validation-memory/report.md) records a failed lateral
improvement gate and unapproved reference trace changes. Performance and candidate
replay pass; neither overrides a behavioral failure. Screening results below are
separate from the full run.

## Candidate and remaining behavioral failure

The candidate retains the 64-second/65-sample, 120-horizon, eight-hex limits.
It requires five observed transitions overall and three in the current window
before extrapolating. The fifth transition invalidates a pending confidence
decision immediately. Once an established transition cadence is missed by more
than one simulation tick, or the unit reaches weapon range, this uninterrupted
contact uses direct pursuit. Contact and lifecycle invalidation reset that state.
Low closing margin and very slow observed motion also use direct pursuit.

Quantization at the lead limit keeps the last legal prediction instead of
alternating between prediction and direct pursuit. Predicted routes use bounded
lookahead through at most eight additional waypoints; every crossed cell must
belong to the original route corridor. Ordinary terrain and braking forecasts
remain authoritative. These fields are transient and included in checkpoint
equality; there is no Lua or persistence change.

Screening found a tradeoff that is not accepted as passing: conservative motion
confidence removes stop/start and damaged-mobility first-opportunity regressions,
but lateral interception improves by 9.12% (269 versus 296 ticks), below the
unchanged 10% gate. The full runner must report that failure. No fixture or gate
was relaxed. Experimental route tie preferences, wider turning admission,
different lead caps, a fitted origin, and smoothing of the direct approach were
not retained.

The complete single-seed weapons-hold screening comparisons are retained for
[established pursuit](screening-existing-pursuit.json) and
[expanded pursuit](screening-extended-pursuit.json). The expanded screen passes
all 24 cases. The established screen's only failed gate is lateral improvement.

The full matrices confirm all 144 expanded cases pass. The preserved reference
has [30 first-opportunity regressions](reference-expanded-comparison.json)
against direct pursuit in that matrix; the candidate removes those failures.
All 192 established pursuit cases satisfy the individual gates, but the aggregate
lateral improvement remains 9.12%, so pursuit acceptance fails. Readiness
fractions are 0.736739 direct / 0.741334 candidate for established pursuit and
0.755648 / 0.757454 for expanded pursuit. Median paired extra approach travel is
zero in both matrices.

This is a tradeoff, not an unconditional capability improvement: vehicle lateral
engagement changes from the previously accepted predictor's 238 ticks to 269
(direct: 296), and damaged vehicles lose an earlier interception advantage
(312 to 355 ticks, matching direct pursuit). Stop/start encounters improve from
239 to 213 ticks for vehicles and 121 to 105 for Mechs. No failed threshold is
waived or described as accepted.

## Correctness-fixture storage

The opt-in `--encounter-temp-root` changes only correctness fixture storage.
A contended throughput probe on this development machine compared four chassis
at 160 ticks: disk took 24.15 seconds and `/dev/shm` took 7.43 seconds. All
[640 committed records matched byte for byte](storage-probe/comparison.json).
The raw [disk](storage-probe/disk.time) and [memory](storage-probe/memory.time)
times are harness throughput measurements, not CPU acceptance timings.
All 36 clearance cases also match across storage locations, totaling another
8,640 records: [late clearance](storage-probe/late_clearance.json),
[alternating clearance](storage-probe/alternating_clearance.json), and
[independent waiters](storage-probe/waiting_controllers.json).

The initial disk-backed full run remains in
`target/autopilot-acceptance/validation/` with an explicit incomplete status.
The completed restarted run used `validation-memory/`, eight encounter workers, and
`/dev/shm` correctness fixtures. Game executable hashes are identical between
the two runs. CPU fixtures retain system-default disk storage; traces and reports
remain on disk. Temporary children are unique and cleaned up without changing
the parent directory or process environment.

## Complete validation results

`just checks` passed, including formatting, generated Lua checks, and the full
Rust test suite: **3,107 passed, two ignored**. The Rust acceptance binary has 24
focused tests, including malformed/missing evidence, cancellation, isolated
storage, provenance tampering, and refusal to waive failed behavioral gates.

All 120 movement, 108 adversarial, and 36 clearance cases pass their behavioral
comparisons. The 192 established pursuit cases fail the aggregate lateral gate;
all 144 expanded pursuit cases pass. All 19 candidate/replay trace groups match:
[329,927 committed tick records](validation-memory/replay-summary.json), with
19 additional byte-identical summary comparisons. Movement and clearance traces
also match the preserved reference. Adversarial and pursuit reference differences
remain unapproved, with exact hashes and first changed cases recorded separately.
No review exception or successful finalization was generated for this candidate.

CPU runs were sequential, without competing builds/tests, tracing, or detailed
attribution: 100 controllers, 100×100 maps, 35 warmup ticks, 60 measured ticks,
three repetitions. Both the six-case guard and separate moving-target workload
pass all timing/resource checks. No weapons-hold p95 regression exceeds 5%.
These are paired measurements, not evidence of a statistically established speedup.

| Scenario | Fire | Autopilot p95 reference / candidate (ms) | Heartbeat p95 reference / candidate (ms) |
| --- | --- | --- | --- |
| Open | hold | 27.349 / 26.987 | 152.448 / 152.665 |
| Open | enabled | 53.732 / 51.361 | 202.841 / 195.203 |
| Obstacles | hold | 32.635 / 31.604 | 165.123 / 164.896 |
| Obstacles | enabled | 54.792 / 53.007 | 220.330 / 200.155 |
| Congestion | hold | 27.090 / 26.752 | 153.194 / 151.701 |
| Congestion | enabled | 47.704 / 47.184 | 202.495 / 199.363 |
| Moving pursuit | hold | 26.672 / 26.014 | 181.147 / 168.900 |
| Moving pursuit | enabled | 48.120 / 46.572 | 193.836 / 186.610 |

The development machine is Linux x86-64, Intel i7-10875H (eight cores/16 threads),
with the powersave governor; rustc 1.98.1, release builds, no RUSTFLAGS override.
[Candidate provenance](validation-memory/candidate-manifest.json) and
[reference provenance](validation-memory/reference-manifest.json) include exact
executable hashes, compiler details, source hashes, and fixture hashes. Raw
[six-case reference](validation-memory/cpu-reference.csv),
[six-case candidate](validation-memory/cpu-candidate.csv),
[moving reference](validation-memory/moving-reference.csv), and
[moving candidate](validation-memory/moving-candidate.csv) CSV retain p50, p95,
maximum, persistence, and resource measurements.

The existing moving-target CPU fixture starts pairs four hexes apart, commonly
already within weapon range. It measures this workload's tracking/replanning cost;
it does **not** establish a 100-controller distant active-prediction stress result.

`validation-memory/` here is a compact evidence export. Its artifact-hash manifest
describes the original sealed run under ignored
`target/autopilot-acceptance/validation-memory/`; binaries, full source snapshots,
large traces, and detailed command logs remain there, not in this export. Earlier
reports and the interrupted disk run are retained. The next acceptance attempt
must restore the lateral gate without reintroducing expanded-suite regressions,
then review reference changes and rerun the immutable workflow.
