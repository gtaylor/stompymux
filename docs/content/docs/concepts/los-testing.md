+++
title = "Line-of-sight testing"
weight = 35
+++

BattleTech line-of-sight (LOS) behavior is tested with Rust unit and
integration tests. `src/btech/los.rs` traces terrain at unit eye heights and
reports blocking terrain, woods, water, smoke, fire, and partial cover. The
perception module (`src/btech/perception/`) then applies the clear-line rule,
the sensor band, sight, active probes and radar to that trace to decide what a
unit can detect.

Run the full suite or a focused integration target from `stompymux-rs/`:

```sh
cargo test
cargo test btech_los_range
cargo test btech_visibility
cargo test btech_vehicle_los
cargo test btech_perception
```

Related scenarios live in `tests/btech_perception.rs`,
`tests/btech_*contact*.rs`, and `tests/btech_scan.rs`. The pure clear-line and
band rules are unit-tested in `src/btech/perception/sight.rs`. Use `cargo test <name>`
to select a specific test by name.

## Adding scenarios

Prefer a small synthetic map that contains only terrain relevant to the rule.
Assert terrain LOS and the resulting perception/contact decision separately
when possible. Use production maps for a few intentional corridors or traversal
invariants, rather than snapshots of every hex pair.

Detection tests should use deterministic inputs. Avoid depending on an
uncontrolled random roll, elapsed time, a running game, or a shared SQLite
database. When expected behavior differs from the existing implementation,
record the intended rule in a focused test and document the discrepancy until
it is resolved.
