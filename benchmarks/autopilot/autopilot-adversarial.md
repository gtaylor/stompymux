# Adversarial autopilot movement

The adversarial suite exercises the production heartbeat, ordinary movement and
shot admission, and real isolated database commits. It adds multiple named
participants without changing the existing ten-scenario suite or Lua contracts.

## Running

```sh
cargo build --release --bin autopilot-encounters
# Full adversarial acceptance matrix: nine scenarios, four chassis, three seeds.
target/release/autopilot-encounters --suite adversarial --ticks 240 --seeds 3 \
  --trace adversarial.jsonl --csv adversarial.csv > adversarial.json
# Repeat with a second trace to verify deterministic outcomes.
python3 tools/compare_autopilot_adversarial.py baseline.json adversarial.json
```

`--suite existing` is the default. `--suite all` runs both suites and produces
combined JSON/CSV rows; existing rows retain their original fields, while new
participant rows identify `schema: 1`. When running both suites, the existing
trace uses the supplied filename and the adversarial trace replaces its extension
with `.adversarial.jsonl`. Unknown suite/scenario selections fail before execution.
The full matrix is opt-in; normal tests use small attribution and scheduler fixtures.

## Fixtures

| Identifier | Behavior |
| --- | --- |
| crossing | An opponent follows timed headings and speeds through ordinary control admission. |
| duel | Both sides use attack orders and autonomous direct fire. |
| occluded | A quarter-speed pursuer follows a near-full-speed opponent behind a three-level wall; contact gaps and reacquisitions exercise the existing memory expiry. |
| bottleneck | Three friendly blockers initially close a corridor; admitted move orders at tick 10 move them past the destination. |
| pursuers | Three allies pursue one opponent through a narrow route; an initial hold heartbeat primes memory, and each attack is submitted only after that unit observes its target. |
| passage | Opposing movement orders share a corridor with passing space. |
| no_passing_space | Three stationary friendly blockers close a corridor permanently; failure to pass is expected and reported separately. |
| mobility_damage | A lower-leg actuator critical or one-point vehicle motive speed loss occurs at tick 30 through existing damage helpers. |
| mixed_arcs | The Mech combines forward/rear mounts; vehicles combine hull/turret mounts. |

Focal chassis are Mech, tracked, wheeled, and hover. Opponents retain the same
AS7-S2 template across comparisons. Initial placement is setup only; scheduled
movement never teleports, repairs, replenishes ammunition, or resurrects units.
Rejected scripted controls (for example, a knocked-down opponent) are counted
without bypassing admission or aborting the fixture.
Combat, sensor, and crew-recovery dice are seeded by participant role and seed.
Setup uses real unit construction, validates the world, and faces initial
attackers toward their opponents before refreshing optical observations.

Native stacking permits some shared hex occupancy. A single stationary opponent
therefore does not establish a genuine blocked corridor. The blocked fixtures
use three known friendly occupants to exercise authoritative crowd admission.
These are local traffic tests, not a reservation or formation system.

## Measurements

Each row names its participant, scenario, chassis and seed. First-shot time and
shot totals use optional per-unit committed-shot counters, not aggregate controller
counts or parsed notices. Rejected heartbeat commits clear attribution and notices.
Timing-only runs do not allocate this per-unit diagnostic map.

Geometry and arc fractions are distinct from firing admission or hit probability.
Arc fraction uses visible sampled ticks as its denominator. Range error is the
continuous distance outside the configured 2–3-hex band; both its sum and sample
count are reported. Travel is continuous in hex units. `settled_distance` measures movement breaking
a five-tick stationary firing-geometry streak; it does not classify all later
travel in a moving-target encounter as wasted. Reversals exclude stationary
ticks; `replans` counts per-unit search restarts, including unsuccessful attempts. Longest no-progress interval uses destination-distance
improvement for fixed movement and travel or usable geometry for pursuit.

Clearance is observed when all three blockers move beyond the focal destination;
recovery measures subsequent progress. Fixed-destination excess travel is reported
only when an initial legal shortest route can be established. A route initially
closed by occupancy has no such baseline and reports null, not zero. Dynamic
pursuit travel is never labeled automatically wasteful.

A unit firing before its destruction in the same committed heartbeat retains that
shot in its count. Contact gaps after initial acquisition and reacquisitions are
reported. The occluded fixture deliberately allows gaps beyond 30 seconds: a
`ContactLost` result after a measured gap is an expected order outcome, not
authorization to follow hidden current coordinates or silently resume a failed
order. The comparison reports these outcomes explicitly.

Sampling ends at destruction, removal, unplacement, or order completion. Rows retain
sampled duration and terminal reason. Blocked participants remain observable for
recovery. Complete BattleTech-state trace checksums include dice, ordered notices and per-unit
shot counts, with timing excluded.

## Local congestion recovery

A search records whether its explored edges encounter known excessive occupancy.
If it exhausts the frontier after encountering a crowd, it stops and retains up
to 64 crowded cells. It checks those cells every five simulation seconds and
allows up to three early retries when one becomes enterable, supplementing the
three timed retries at 30-second intervals. Early failures do not postpone timed
deadlines. Waiting performs no search expansions and retains no exhausted
frontier. Static-unreachable searches that never encounter crowding keep their
existing failure behavior. Planning-context changes invalidate the watches.
Permanent obstruction still reaches a blocked outcome; there is no shared
passage reservation or indefinite retry loop. See [responsive congestion
recovery](autopilot-clearance.md) for the current policy and additional opt-in
fixtures. The results below record the earlier timed-retry implementation.

## Validation evidence

Results and remaining limitations are recorded with the artifacts after the paired
runs. Behavioral comparisons distinguish expected permanent obstruction from
unexpected failures and keep failed gates visible. CPU results are separate from
traced encounter runs. All benchmark databases are temporary isolated fixtures.

### Seeded results (2026-09-23)

The reference uses the same fixtures and instrumentation with only congestion
retry disabled; it is not a different weapon or steering policy. All 108
encounters (nine scenarios × four chassis × three seeds, 240 ticks each) passed
the comparison gates. The 288 participant summaries and comparison are in
[`autopilot-adversarial-artifacts`](autopilot-adversarial-artifacts/README.md).

- Clearing-corridor focal completions improved from **0/12 to 12/12**. Mechs
  finish at tick 100 and all vehicle classes at tick 145. Each focal records
  progress one tick after all blockers clear the destination at tick 82.
- Passing-space focal completions remain **12/12**, with both sides completing.
- All **36 pursuers fire by tick 3**, with no blocked attacker orders.
- The unchanged 120-case movement matrix passes its existing gates, and all
  **28,800 tick records match the prior movement trace byte for byte**.
- The adversarial candidate replay matches all **25,920 committed tick records
  byte for byte**, including BattleTech state/dice, ordered notices and shot
  attribution. The final matrix combines 96 unchanged cases from the full run
  with a separately rerun 12-case pursuit slice after correcting its initial
  sensor-admission setup. Reference and replay use the same composition.
- `just checks` passed **3,047 tests**, with one ignored, across 21 groups,
  including formatting and Lua type/doc verification. The subsequent
  fixture-only pursuit-admission correction passed the CLI test and release
  replay. Eight Python comparator tests pass independently.

| Combat scenario | Focal first shot (ticks) | Focal usable arc fraction, min–max |
| --- | ---: | ---: |
| crossing | 1 | 1.000–1.000 |
| duel | 1 | 1.000–1.000 |
| occluded | 1 | 1.000–1.000 |
| pursuers | 2 | 1.000–1.000 |
| mobility_damage | 1 | 1.000–1.000 |
| mixed_arcs | 1 | 0.821–1.000 |

These fixtures start within engagement distance to stress continued pursuit,
traffic and changing readiness. Their first-shot results do not demonstrate
long-distance approach performance. Range-band exposure, range error, reversals,
travel and all participants' individual metrics remain in the raw summaries.
The maximum firing-settle break distance among combat focal units is 0.0825
hexes; dynamic pursuit travel is reported separately.

Remaining limits are explicit: all 12 permanent-crowd focal orders block;
all 12 occlusion cases eventually expire contact after measured gaps of at
least 30 ticks; eight duel opponents become `UnitUnavailable` through combat.
There is no automatic retry of expired hostile contact and no coordinated
passage negotiation. These outcomes are listed individually by the comparator,
not counted as successful movement completions.

### Short CPU regression guard

Both frozen release executables ran sequentially with 100 controllers on
100×100 maps, 35 warmup ticks, 60 measured ticks and three repetitions per
case: **1,080 measured heartbeats per executable**. No builds, tests, traces or
detailed attribution competed with these runs. Resource, commit and controller
service assertions passed; every reported deterministic counter, including
shots, expansions, renewals and completion/block counts, matched.

| Scenario | Fire | Autopilot p95 before → after (ms) | Whole-heartbeat p95 before → after (ms) |
| --- | --- | ---: | ---: |
| open | hold | 37.50 → 36.71 | 186.25 → 180.98 |
| open | opportunistic | 64.37 → 63.35 | 230.79 → 213.90 |
| obstacles | hold | 41.22 → 39.44 | 199.64 → 178.34 |
| obstacles | opportunistic | 66.25 → 62.90 | 249.88 → 235.41 |
| moving_congestion | hold | 36.61 → 35.30 | 185.11 → 170.80 |
| moving_congestion | opportunistic | 58.96 → 57.06 | 229.50 → 213.89 |

No case regressed. The observed 1.6–5.1% autopilot p95 reductions are treated
as run variation, not an established optimization gain: the new retry path
is not exercised materially by this standard workload, whose counters match.
The raw aggregate CSV includes p50 and persistence timings. All six cases
meet the current [benchmark guards](autopilot-benchmark.md): below 75 ms for
firing and below 50 ms for weapons hold. The firing guard was raised after
this run; the measurements are unchanged and still exceed the former 50 ms
firing target. This short guard run does not establish full-run acceptance. The documented machine is an Intel Core i7-10875H with
Rust 1.98.1; full details and executable hashes accompany the results.
