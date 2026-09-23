# Adversarial movement artifacts

See [the report](../autopilot-adversarial.md) for fixture and metric definitions.
All databases were temporary isolated copies of the test fixture database.

- `matrix-reference.{json,csv}` and `matrix-candidate.{json,csv}`: all 288
  participant summaries, covering 108 encounters and 25,920 ticks per matrix.
- `comparison.json`: explicit acceptance failures (none), changed rows and
  expected limitations. `existing-comparison.json`: unchanged 120-case gates.
- `provenance.json`: compiler command, revision, dirty-workspace note, fixture
  and relevant source hashes, frozen executable hashes and replay hashes.
- `reference-policy.txt`: the single disabled congestion-retry branch in the
  otherwise identical reference implementation.
- `machine.txt`: compiler and development-machine details.
- `cpu-{reference,candidate}.csv` and `cpu-comparison.json`: isolated six-case
  short performance guard, 1,080 measured heartbeats per executable. These
  are aggregate benchmark output, not raw per-tick samples.

The final matrix retains the unchanged 96 cases from each full v1 run and
replaces the 12 pursuit cases with the v2 fixture's ordinary sensor-admitted
attack submission. Each slice has independent seeded worlds. Both reference
and candidate received this fixture correction. Replay uses the identical
composition; raw trace lines were retained without reserialization.

Large raw traces, build/check logs and frozen executables are retained locally
under `/tmp/autopilot-adversarial/`; these temporary files are not repository
artifacts. Final traces are `matrix-{reference,candidate,replay}.jsonl` and
`existing.jsonl`. `assemble.py` there records the slice assembly.

Reproduce correctness with:

```sh
cargo build --release --bin autopilot-encounters --bin autopilot-bench
# A fresh full final-fixture matrix needs no slice assembly.
target/release/autopilot-encounters --suite adversarial --ticks 240 --seeds 3 \
  --trace candidate.jsonl --csv candidate.csv > candidate.json
target/release/autopilot-encounters --suite adversarial --ticks 240 --seeds 3 \
  --trace replay.jsonl > replay.json
cmp candidate.jsonl replay.jsonl
python3 tools/compare_autopilot_adversarial.py reference.json candidate.json
python3 -m unittest discover -s tools -p test_compare_autopilot_adversarial.py
```

Build the reference from a separate source copy with the documented branch
change and its own Cargo target directory. Freeze executables before switching
source roots; reusing one target directory can reuse the other package's binary.
For the CPU guard run each frozen executable sequentially with
`--warmup 35 --ticks 60 --repetitions 3`, without tracing or detailed attribution.
