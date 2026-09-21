+++
title = "Testing"
description = "Run unit and integration tests and inspect failed integration fixtures."
weight = 30
+++

The test suite is divided into unit and integration groups:

```sh
just test-unit
just test-integration
just test
```

`just test` runs both groups and remains part of `just agent-checks`. The same
groups can be selected directly with CTest's `unit` and `integration` labels.

Unit tests also have topical labels. Run all BattleTech tests or a narrower
topic with, for example:

```sh
just test-unit-topic btech
just test-unit-topic btech-sensors
just test-unit-topic mux-content
```

The available topics are `btech-autopilot`, `btech-combat`,
`btech-contracts`, `btech-sensors`, `btech-systems`, `mux-content`,
`mux-network`, `mux-server`, `mux-state`, and `mux-support`. The `btech` and
`mux` umbrella labels select all topics in their respective subsystem.

Integration tests that need game content run against a private copy of the
stripped game tree in `tests/fixtures/game`. Suite-specific files from
`tests/fixtures/integration` are applied as overlays. Each server test starts
without a database so the normal server bootstrap creates one using the
configuration defaults.

Successful temporary game directories are removed automatically. If a suite
fails, its output includes an absolute path such as:

```text
Integration test artifacts retained at: /tmp/btmux-libuv-tcp.ABC123
```

That directory contains the generated configuration, database, logs, and
fixture content from the failed run. Set `BTECH_KEEP_TEST_DIRS=1` to preserve
directories from successful integration runs as well.
