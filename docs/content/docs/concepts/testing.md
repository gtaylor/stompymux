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
suite. A bare test-name filter such as `cargo test btech_los` still builds
every test binary before filtering, so prefer selecting the target as well.
The scenario files under `tests/` are grouped into suite binaries under
`tests/suites/`, and tests that run the server executable live in the `cli`
target under `tests/cli/`; `just test-scenario` locates the right suite for
you:

```sh
just test-unit btech::los             # unit tests in src/ matching a name
just test-scenario btech_los_range    # one scenario file from tests/
just test-scenario btech_visibility perception
just test-suite btech_08              # one whole suite
just list-scenarios                   # map of suites to scenario files
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

Server diagnostics in integration tests go through the test harness's output
capture: a passing test prints nothing, and a failing test prints the log lines
that led up to its failure. Set `RUST_LOG` to change what is recorded, for
example `RUST_LOG=debug just test-scenario btech_status`. To assert on emitted
events, install `stompymux_rs::logging::Capture` for the test's thread and
inspect its text.
