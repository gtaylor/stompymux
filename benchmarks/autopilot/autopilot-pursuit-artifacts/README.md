# Pursuit validation artifacts

See [the report](../autopilot-pursuit.md) for acceptance results and limitations.

- `validation.json`: check counts and explicit overall acceptance failure.
- `trace-hashes.json`: SHA-256 hashes for all 34 retained direct/candidate/replay trace files.
- `provenance.json`: machine, compiler, source and executable hashes, fixture storage, and protocols. The preserved reference predates this follow-up; direct pursuit uses the candidate executable's harness-only switch.
- `direct-pursuit.json`, `candidate-pursuit.json`: all 192 runs per policy (96 cases × two firing modes), including contact episodes and terminal outcomes.
- `direct-episodes.json`, `candidate-episodes.json`, `replay-episodes.json`: augmented contact episodes with reconciled committed counters and reacquisition delay. Candidate/replay reports are byte-identical.
- `pursuit-comparison.json`: strict behavioral gates, unmatched outcomes, and paired excess approach distance. A populated `failures` array means behavioral acceptance failed.
- `replay-comparison.json`: SHA-256 checks of complete candidate/replay trace files, including gameplay digests and measurements.
- `regression-traces.json`: per-case trace comparison with the prior clearance implementation. Changed cases are not described as byte-identical.
- `existing-candidate.json`, `adversarial-candidate.json`, and clearance scenario summaries: regression outcomes. Comparator reports retain failures and changes.
- `cpu-*.csv`: raw sequential release benchmark output, with normal-filesystem isolated SQLite databases. No tracing or detailed attribution was enabled.
- `cpu-comparison.json`: autopilot and whole-heartbeat p95 comparisons, absolute guards, and flags requiring hold-case repetition.
- `cpu-repeats.json` and `cpu-repeat-*.csv`: sequential open/hold and obstacles/hold repeat pairs; the open >5% regression reproduced.
- `fixture-smoke.json`: setup validation of the frozen ordinary-patrol short-occlusion fixture.

Large raw traces, executable copies and build/check logs are retained in `/tmp/autopilot-pursuit/` for this development run. Their hashes are recorded here, but these scratch files are not a portable archive. Earlier reports and artifacts remain unchanged.
