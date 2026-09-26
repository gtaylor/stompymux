# Autopilot benchmarks and validation reports

This directory retains development measurements and reproducibility evidence.
These files are repository documentation, not Hugo site content. Reports record
the results of their particular run, including failed acceptance gates; they
are not promises about current performance.

- [Heartbeat benchmark](autopilot-benchmark.md)
- [Observation and combat CPU](autopilot-cpu.md)
- [Combat transaction CPU](autopilot-combat-cpu.md)
- [Combat movement](autopilot-movement.md)
- [Adversarial movement encounters](autopilot-adversarial.md)
- [Congestion clearance](autopilot-clearance.md)
- [Predictive pursuit](autopilot-pursuit.md)
- [Pursuit recovery and acceptance follow-up](autopilot-acceptance-followup.md)
- [Pursuit acceptance readiness fixes](autopilot-readiness-fix.md)
- [Adaptive pursuit redesign and failed acceptance evidence](adaptive-pursuit/README.md)
- [Sustained engagement recovery and remaining CPU regression](adaptive-pursuit/recovery/README.md)

Each report links its retained CSV, JSON, and provenance artifacts. Keep earlier
measurements intact when adding a new report. Large scratch traces and binaries
referenced by historical reports may live outside the repository.

See [autopilot design and audits](../../design/autopilot/README.md) for internal
contracts, and [the documentation site](../../docs/README.md) for the Hugo layout.

Current validation commands: [Rust acceptance runner](autopilot-acceptance.md).
