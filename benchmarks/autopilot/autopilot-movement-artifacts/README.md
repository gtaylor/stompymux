# Combat movement evidence

- `encounters-baseline.json` / `.csv`: previous runtime policy, 120 seeded encounters, 240 ticks each.
- `encounters-candidate.json` / `.csv`: improved movement policy on the same fixture matrix.
- `comparison.json`: first-shot, usable-arc, settled-distance, and controller-state gates.
- `trace-comparison.json`: complete-state/dice checksum and ordered-notice replay comparison.
- `cpu-baseline.csv` / `cpu-candidate.csv`: existing six-case 100-controller, 100x100 CPU guard; three repetitions, 35 warmup and 60 measured ticks. Tracing and detailed attribution are disabled.
- `cpu-comparison.json`: six-case before/after timings and shot counts; the CPU guard shows a movement-phase regression.
- `cpu-hold-repeat-baseline.csv` / `cpu-hold-repeat-candidate.csv`: sequential open/hold confirmation of the regression.
- `checks.json`: repository checks and follow-up verification.
- `executables.sha256`, `sources.sha256`, `reference-runtime.sha256`: retained executable/source identities.
- `machine.txt`: development machine and compiler details.

The checkout began at the revision in `source-revision.txt`, with unrelated macro,
configuration, and Lua work already present. That work was preserved. The starting
working diff, reference runtime snapshot, binaries, complete traces, build/check
logs, and intermediate measurements are retained under `/tmp/autopilot-movement`.
The reference executable uses the same encounter instrumentation with the prior
runtime policy; its helper visibility adjustment changes no decisions.

The scripted moving-target fixture initially tried to restart an already destroyed
target. Its relocation guard was corrected. The complete reference run never hit
that failure. A complete corrected candidate run was compared with a replay
consisting of 20,160 already completed matching records and rerun independent
moving/attack-move/disabled-turret encounters. A subsequent complete run verifies
that the turret-offset normalization fix preserves all 28,800 accepted encounter
records. The 100-controller benchmark separately exercises the offset edge case.

Reproduce movement outcomes with:

```sh
cargo build --release --bin autopilot-encounters --bin autopilot-bench
target/release/autopilot-encounters --ticks 240 --seeds 3 --trace first.jsonl --csv first.csv > first.json
target/release/autopilot-encounters --ticks 240 --seeds 3 --trace second.jsonl > second.json
cmp first.jsonl second.jsonl
python3 tools/compare_autopilot_movement.py BASELINE.json first.json
```

Reproduce the CPU guard with no competing builds, tests, tracing, or detailed
attribution:

```sh
target/release/autopilot-bench --warmup 35 --ticks 60 --repetitions 3
```

The short CPU guard is not a replacement for the earlier full 18,000-heartbeat
performance acceptance run. Earlier combat CPU reports remain intact.
