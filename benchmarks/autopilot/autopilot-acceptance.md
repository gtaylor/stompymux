# Rust autopilot acceptance

The `autopilot-acceptance` binary orchestrates isolated builds, deterministic
encounters, replay, comparisons, and CPU guards without a Python runtime.
Historical reports retain their original commands; their former Python tools are
archived under `rust-acceptance/legacy-tools/` and are not current dependencies.

## Capture and run

```sh
just autopilot-acceptance capture --output target/autopilot-acceptance/reference
# Make the intended production changes, preserving the reference directory.
just autopilot-acceptance run --baseline target/autopilot-acceptance/reference --output target/autopilot-acceptance/candidate
```

Both output directories must be new. Capture includes executable hashes, a source
snapshot, the working-tree patch, compiler information, and machine details.
Full runs invoke `just checks`, then encounter jobs (default four workers), then
strictly sequential CPU measurements. Do not run competing builds/tests during
the CPU phase. Use `--jobs N` for encounter concurrency. `--behavior-only` skips
CPU work and explicitly leaves performance acceptance unevaluated.

For faster correctness runs on a machine with sufficient temporary memory, pass
`--encounter-temp-root /dev/shm` (or another existing temporary directory).
Every encounter gets its own child directory, cleaned up on completion. This
option affects correctness fixtures only: timed CPU fixtures keep the system
default temporary storage. The selected location and worker count are recorded
in `run-settings.json`. Traces and reports remain in the output directory.

The runner checks complete matrices, consecutive trace ticks, byte-identical
replay, behavioral gates, and unchanged CPU thresholds (75 ms firing, 50 ms hold).
The CPU protocol is 100 controllers, 35 warmup ticks, 60 measured ticks, three
repetitions, with tracing disabled. Both the six established CPU cases and the
two moving-target cases run. A hold regression above 5% is an investigation
failure; it is never silently retried away.

Encounter and CPU runs explicitly select Adaptive for both the captured reference
and candidate/replay, and direct pursuit for direct comparisons. The CPU executable
supports `--pursuit-policy adaptive|direct`; encounters use
`--pursuit-policy adaptive` with `--direct-pursuit` for direct comparisons.
Both support `--policy-metadata PATH`.
Metadata records requested/resolved policy after checking every heartbeat; a
mismatch fails the run without changing existing CSV columns. Captures record
selector capabilities and `pursuit_policy: "adaptive"`. Runs reject captures without
that explicit identity before building the candidate; capture a fresh reference
with the current tool. G is removed from runtime and CLI selection. Historical
reports and measurements remain valid records of their captured executables;
use their preserved tools to reproduce historical protocols. Removing G does not
reclassify any earlier failed CPU verdict as passing.

Interruptions terminate child process groups and leave partial evidence. Nonzero
exit status, `status.json`, `report.json`, and `report.md` distinguish incomplete
work, failed gates, and completed acceptance. Do not label behavior-only success
as full acceptance. A stale or modified reference executable is rejected.

## Review intentional behavior changes

Reference traces must remain identical unless a review file accounts for their
exact hashes and every changed case. Generate explanations from the retained
`reference-*.trace-diff.json` evidence. Append a reviewed verdict with the preserved
tool, without repeating measurements or modifying the original verdict:

```sh
target/autopilot-acceptance/candidate/acceptance-tool finalize --run target/autopilot-acceptance/candidate --reviewed-changes reviewed-changes.json --output target/autopilot-acceptance/reviewed
```

The output must be new and outside the sealed measurement directory. Finalization
verifies every measurement hash and refuses behavioral, replay, or CPU failures.
Alternatively, supply `--reviewed-changes PATH` when running a new measurement.
Review entries are keyed by scenario/suite suffix, with
`reference_sha256`, `candidate_sha256`, and a `reasons` object keyed by the exact
serialized case identity from `changed_cases`. Every reason must be nonempty.
Changed hashes invalidate the review. Candidate replay can never be waived.

## Independent tools

```sh
just autopilot-acceptance compare movement baseline.json candidate.json
just autopilot-acceptance compare adversarial baseline.json candidate.json
just autopilot-acceptance compare pursuit direct.json predictive.json
just autopilot-acceptance compare trace first.jsonl replay.jsonl
just autopilot-acceptance compare cpu reference.csv candidate.csv
just autopilot-acceptance episodes summary.json trace.jsonl
just autopilot-acceptance timeline trace.jsonl
```

`compare` prints JSON and exits nonzero on failed gates or invalid input.
`episodes` checks monotone counters, visibility-episode attribution, contiguous
ticks, and reconciliation with the summary. Geometry, readiness, and actual shots
remain separate; no metric draws combat dice.

## Expanded pursuit

`autopilot-encounters --suite pursuit_extended --seed-start 4 --seeds 3 --ticks 900`
runs 144 cases: six patterns, four chassis, three seeds, hold and assigned-target
fire. Existing default suite selections remain unchanged; explicit `all` includes
these cases. Parameters are frozen in
[the fixture manifest](../../design/autopilot/pursuit-extended-fixtures.json).

Large traces and binaries stay in ignored `target/autopilot-acceptance/`.
Retain compact final reports, CSVs, hashes, and unresolved limitations under
`benchmarks/autopilot/`, outside Hugo content. Small Rust tests for tools and
behavior run in `cargo test`; the large encounter matrices remain opt-in.

`timeline` summarizes committed pursuit reasons, goal changes, motion diagnostics,
and first prediction per case. Where complete gameplay digests are present, it
also hashes each case's ordered digest sequence so diagnostic-only trace changes
can be distinguished from gameplay changes. Motion sample counts make absent
fields in older traces explicit. These categories describe observations, not
causal proof, and never replace exact candidate replay comparisons.
