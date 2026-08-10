+++
title = "Line-of-sight testing"
weight = 35
+++

The line-of-sight (LOS) regression suite is split into four CTest targets so a
failure identifies the affected layer:

* `btech_los_trace` checks hex traversal, tie-breaking, and bounded exhaustive
  path invariants.
* `btech_los_geometry` checks unit eye heights, interpolated elevation,
  partial cover, terrain flags, range, altitude, and the water/air interface.
* `btech_los_sensors` checks sensor range and contact rules, ECM and signature
  systems, seismic movement, radar clearance, and sensor to-hit modifiers.
* `btech_los_maps` traces representative corridors on flat, mountainous, and
  water-heavy production maps.
* `btech_los_cache` checks directional observer/target cache entries, flag
  decoding, and observer invalidation without disturbing reverse visibility.

Run only this suite with:

```sh
cmake --build .build --target btech_los_trace_test btech_los_geometry_test \
  btech_los_sensors_test btech_los_maps_test btech_los_cache_test
ctest --test-dir .build -L los --output-on-failure
```

## Adding scenarios

Prefer a small synthetic map that contains only terrain relevant to the rule.
Assert the LOS flags and sensor result separately when possible. Production-map
tests should enforce traversal invariants or a few intentional corridors rather
than snapshotting every hex pair.

Detection tests must use deterministic inputs. Do not make a test depend on a
random roll, elapsed time, a running game, or the live SQLite database.

## Known divergences

Use `los_expect_divergence_int` only when repository documentation or an
explicit rule establishes an intended value but the current implementation has
a known legacy result. The helper accepts the legacy result while failing an
XPASS. When an XPASS occurs, review the behavior, replace the divergence with a
normal expectation, and update any related help or game documentation. Do not
use a divergence merely to make an unexplained failure pass.
