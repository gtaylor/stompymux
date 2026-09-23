# Combat CPU measurement artifacts

- `full.csv` / `full.time` / `acceptance.json`: complete 18,000-heartbeat acceptance,
  elapsed time, resource checks and explicit timing-target misses.
- `checks.json`: formatting, Lua freshness and Rust test results.
- `baseline-short.csv` / `final-short.csv`: paired six-case timing (35 warmup,
  60 measured ticks, three repetitions).
- `trace-comparison.json`: byte-for-byte comparison of all 1,710 committed records.
- `attribution.json`: named combat profiles at successive optimization stages.
  Inclusive categories overlap; profiling times are not acceptance measurements.
- `executables.sha256`: preserved release executable identities.
- `compiler.txt` / `machine.json`: build and host details.
- `profile.gdb` / `gdb-stacks.txt`: separate baseline stack-sampling procedure/output.
- Intermediate CSV files retain accepted and rejected experiments. `candidate-v2`
  was rejected and is not the final implementation.

Large executables, full source snapshots and JSONL traces are retained locally in
`/tmp/autopilot-combat-cpu`; the report records their identities and comparisons.
The baseline source is commit `1dce945ed34c6f62dca5c510c44242e25a4cb3cf`.
