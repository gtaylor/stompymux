+++
title = "Testing"
description = "Run Rust unit and integration tests and inspect failed fixtures."
weight = 30
+++

The Rust test suite contains unit tests beside their implementations and
integration tests in `tests/`. Run the complete suite from `stompymux-rs/`:

```sh
cargo test
```

`just test` runs the same command. `just checks` also runs Rust formatting,
Lua formatting, generated Lua type and API documentation checks, and the test
suite. To run a narrower test, use a test-name filter or select an integration
test target:

```sh
cargo test btech_los
cargo test btech_los_range
cargo test btech_visibility
```

The BattleTech LOS scenarios are Rust tests, including `btech_los_range`,
`btech_visibility`, `btech_vehicle_los`, and perception and contact tests. Focused
unit tests cover rules close to their implementations. Prefer small synthetic
maps and deterministic inputs when adding a LOS case; assert terrain tracing
and perception separately where the rule distinguishes them.

Integration tests use isolated game fixtures under `tests/fixtures/game` and
helpers under `tests/support/`. They exercise the Rust server, Lua APIs,
SQLite persistence, and network behavior. A failing test reports its assertion
and any fixture or temporary directory it retains for inspection.
