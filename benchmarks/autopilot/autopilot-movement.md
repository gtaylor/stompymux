# Ground autopilot combat positioning

Combat movement uses preferred firing regions, prospective terrain visibility,
ordinary ground-motion forecasts, and admitted hull/torso/turret controls. No Lua
contract or persistence schema changes are required. Search and steering state
remain transient and participate in heartbeat rollback.

## Policy

An explicit attack range overrides the configured preferred band. Without either,
the policy estimates the aggregate direct-fire effectiveness of intact, supplied
ordinary weapons. It chooses the farthest integer range within 90% of the best
score, including the adjacent nearer range only when that range also qualifies.
Temporary recycle and heat do not change this positioning preference.

Engagement searches use an A* annulus with an admissible hex-distance lower bound.
A candidate firing cell must offer terrain visibility and range for an available
weapon. The preferred region is tried first; only proven unreachability enables
the wider usable-range fallback. Budget exhaustion remains pending. Attack-move
routes and destinations are restricted to six hexes from the diversion origin.
Lost contacts retain the existing remembered-position investigation and expiry.

The scheduler retains its 256 expansions per controller, 25,600 global expansions,
and one-million-record limits. Prospective geometry has separate limits of 16
checks per controller and 1,600 globally per movement tick. Work is served in the
existing rotating deterministic order. Completed searches release their frontiers.

Steering looks ahead along straight route segments and forecasts up to 32 ordinary
motion steps per throttle candidate, including braking. Every crossed hex is
checked for legal traversal. Reverse travel is available when it preserves useful
arcs, with a four-tick direction latch and a stop before changing direction.
Turning and braking count as progress for stuck detection. Live movement remains
authoritative for hazards, occupancy and dice.

After movement, observed targets are aligned using serviceable weapon-weighted
arc scores. Equal scores retain existing controls; torso and working turret
adjustments take precedence over stationary hull turns. Jammed or locked turrets
retain their hull-relative offset. Successful player controls still trigger manual
takeover; internal controls retain crew-health and mechanical admission checks.

## Reproduction

```sh
cargo build --release --bin autopilot-encounters --bin autopilot-bench
autopilot-encounters --ticks 240 --seeds 3 --trace encounters.jsonl --csv encounters.csv
# Filter one fixture, retaining all four supported chassis:
autopilot-encounters --scenario obstacle_pursuit --ticks 240 --seeds 3
# CPU guard: tracing and detailed attribution disabled.
autopilot-bench --warmup 35 --ticks 60 --repetitions 3
```

The runner uses private temporary game roots and databases. Its fixed seeds cover
in-range and long-range approach, close retreat, facing away, corner approach, wall detour, unreachable
preferred range, moving target, attack-move, and damaged turret encounters for
Mechs, tracked, wheeled and hover vehicles. The fixture named `jammed` applies a permanent turret rotation lock; both locked and jammed turrets use the same hull-alignment fallback, and ordinary admission rejects rotation in either state. The moving target uses a scripted
placement event every 30 ticks; attack-move moves its opponent beyond the leash at tick 60. These scripted targets are not second autonomous controllers.

`time_to_engage` counts ticks until the first committed autonomous shot.
`geometric_ticks` requires a visible, living target, terrain LOS, weapon range and
at least one usable arc. `ready_ticks` additionally requires mechanical weapon readiness and room under the configured projected heat ceiling, or an actual committed shot that tick. It is not a full dry-run shot admission check. `arc_fraction` weights usable in-range arcs by nominal damage
and missile count over visible ticks. These are distinct from hit probability.

Distance is continuous travel in hex units. Settling requires five consecutive
stationary ticks with usable geometry in the configured discrete range band (any usable range in the fallback fixture);
`settled_distance` counts subsequent travel until a scripted target relocation resets the settling interval.
Reversals ignore stationary ticks. Overshoots count crossing from one side of the
band to the other. Shots, damage, replans and final controller state are also
reported, together with settled ticks, ticks in the preferred band, and final observed range. The settled-tick count prevents a zero movement result from hiding failure to settle. Per-tick traces hash complete serialized BattleTech state and ordered
autonomous notices, including dice state; timings are excluded.

## Results

The comparison uses 120 encounters: ten scenarios, four chassis, three fixed seeds,
and 240 committed ticks each. All movement gates pass: no first-shot regression
above one tick, no arc-uptime regression above two percentage points, no blocked
candidate controllers, and zero post-settle travel. Stationary fixtures have at
least 117 settled ticks. Thirty-three encounters now engage where the reference
never engaged. The two candidate traces match across all 28,800 tick records,
including serialized state/dice checksums and ordered notices.

Means across all chassis and seeds follow. Extra approach travel is approximately
0.2 hex inside the same discrete arrival band; it is not post-settle drift.

| Scenario | Usable arcs before → after | Travel before → after (hexes) |
| --- | ---: | ---: |
| approach | 100.0% → 100.0% | 3.77 → 4.00 |
| long_approach | 89.7% → 92.0% | 6.77 → 6.99 |
| close | 2.1% → 100.0% | 0.93 → 0.90 |
| behind | 0.0% → 99.4% | 0.00 → 0.00 |
| corner | 25.0% → 100.0% | 0.76 → 0.98 |
| obstacle_pursuit | 61.0% → 97.3% | 5.31 → 5.23 |
| fallback | 4.6% → 100.0% | 7.34 → 0.00 |
| moving | 100.0% → 100.0% | 4.77 → 5.01 |
| attack_move | 99.4% → 100.0% | 8.91 → 9.00 |
| jammed | 0.0% → 93.1% | 0.00 → 0.00 |

Facing-away Mechs engage in five ticks and vehicles in one, compared with no
engagement before. Vehicles negotiating the wall fire on tick one instead of
15. A disabled turret requires a hull turn and engages in 21 ticks instead of
remaining unable to fire. Attack-move resumes and finishes after the scripted
target leaves its leash.

The 100-controller guard exposed a floating-point edge case in shared turret
control: `rem_euclid` can round a tiny negative offset to 360. Canonicalizing that
endpoint to zero preserves the strict facing validator. An admitted-control
regression checks both world validity and absence of manual takeover.

The diagnostic moving-target script also needed a guard against relocating and
restarting a destroyed target. One improved Mech encounter kills its target before
the final scripted relocation; that order now completes normally in the fixture.

Raw summaries, executable/source hashes, machine/compiler details, and comparison
results are in [autopilot-movement-artifacts](autopilot-movement-artifacts/README.md).
CPU guard results and final verification are recorded below.


## CPU guard and verification

The isolated release guard used 100 controllers on 100×100 maps, three repetitions,
35 warmup and 60 measured ticks per case (1,080 measured heartbeats). No tracing,
detailed attribution, builds, or tests ran alongside it. All commit, expansion,
search-memory and controller-service assertions passed. Every case retained 100
enabled controllers with a maximum service delay of one tick.

| Scenario | Fire | Autopilot p95 before → after (ms) | Whole heartbeat p95 before → after (ms) | Shots before → after |
| --- | --- | ---: | ---: | ---: |
| open | hold | 25.744 → 36.762 | 195.002 → 185.416 | 0 → 0 |
| open | opportunistic | 43.945 → 62.576 | 226.806 → 226.736 | 252 → 439 |
| obstacles | hold | 32.575 → 41.046 | 198.015 → 198.013 | 0 → 0 |
| obstacles | opportunistic | 53.675 → 64.516 | 233.586 → 249.674 | 328 → 424 |
| moving_congestion | hold | 26.023 → 36.213 | 186.080 → 182.349 | 0 → 0 |
| moving_congestion | opportunistic | 48.315 → 58.353 | 220.831 → 226.533 | 312 → 437 |

**The CPU nonregression guard is not met.** Weapons-hold autopilot p95 increases
26–43%, concentrated in movement: open-map movement p95 rises from 4.687 to
15.588 ms, while observation remains 21.086 versus 21.182 ms and combat remains
21.478 versus 21.515 ms. This identifies movement processing as the remaining
cost, not a combat or observation regression. The new steering loop forecasts
ordinary motion and braking for each candidate throttle; attribution to individual
helpers within that loop has not yet been measured. Whole-heartbeat hold p95 is
flat or lower in this short run, which does not negate the autopilot-phase regression.
A sequential open/hold repeat confirms the increase: autopilot p95 25.593 →
35.420 ms (+38.4%), movement 4.760 → 15.033 ms, observation 20.710 →
20.715 ms, and combat 21.063 → 21.076 ms. Whole-heartbeat p95 for that
repeat was 195.243 → 169.610 ms. Raw repeat CSVs are retained with the report.
Firing cases also perform more shots, so their timing changes cannot be treated
as a comparison with identical gameplay work. All three firing p95 values remain
above 50 ms. This movement change does not close the prior combat CPU gap.

Repository verification: `just checks` passed 3,044 tests with one ignored test,
plus formatting and generated Lua type/document checks. The final executable
reproduced all 28,800 accepted encounter trace records and the complete summary
byte for byte after the turret normalization fix. The behavior gates pass; the
CPU regression remains a documented limitation and a target for follow-up work.
