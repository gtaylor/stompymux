# Clearance recovery artifacts

See [the report](../autopilot-clearance.md) for semantics and fixture definitions.

The original reference executables were frozen before execution changes. The
instrumented encounter reference uses the final fixtures and timed retry state,
with the two early-eligibility conditions in `congestion.rs` disabled. This
allows identical watched-cell measurements without early retry behavior. CPU
comparisons use the original frozen executable so instrumentation and watch
storage overhead are not hidden in the baseline.

The standard matrices remain 120 existing movement encounters and 108
adversarial encounters. Three additional explicitly selected scenarios add
36 encounters. All use four chassis, three seeds, and 240 committed ticks.
Timing samples exclude trace and detailed attribution and run sequentially
without competing builds, tests or encounters.

Raw traces, frozen executables and build/test logs are retained locally in
`/tmp/autopilot-clearance/`. They are temporary validation files rather than
repository artifacts. Summaries, comparisons, hashes and measured timings are
retained here. All databases are disposable fixture copies.

Final acceptance:

- `existing-candidate.json` / `adversarial-candidate.json` and their comparison
  reports cover the standard suites against the prior movement implementation.
- `{late_clearance,alternating_clearance,waiting_controllers}-{reference,candidate}.{json,csv}`
  contain final paired summaries; corresponding comparison files retain all gates.
- `recovery-regression.json` records the rising-timer failure and corrected outcome.
- `waiting-benchmark.txt` contains the isolated, release-mode 100-waiter result.
- `provenance.json` identifies final source and executable hashes.

Accepted full traces are `accepted-{candidate,replay}.jsonl` (movement) and
`accepted-{candidate,replay}.adversarial.jsonl`; new scenario traces use
`<scenario>-{candidate,replay}-v2.jsonl`, all under the temporary artifact root.
The final traces were produced by complete runs of the same final executable.
Correctness runs used RAM-backed temporary SQLite; CPU runs use normal storage.
Earlier intermediate traces and failed-fixture diagnostics remain in the
scratch directory and are not the accepted measurements.

`cpu-reference.csv`, `cpu-candidate.csv` and `cpu-comparison.json` contain
the six-case short CPU guard and explicit counter differences. These are the
benchmark's aggregate CSV outputs, not per-tick timing samples.
`standard-candidate.csv` contains both standard encounter suites.
All 264 encounter gates passed, all 63,360 tick records replayed identically,
and the final repository checks passed 3,053 tests (two ignored).
