# Ground autopilot architecture

The controller belongs to a BattleTech unit and stores intent rather than
movement commands. Trusted Lua and a future tactical director submit the same
typed orders. A one-second simulation tick observes the battlefield, advances a
bounded route search, and executes actions through ordinary unit mechanics.

## Durable contract

`BtechState.controllers` is keyed by unit ID. Each controller persists its
configuration, management revision, next order ID, active order, FIFO queue,
pause or block state, and bounded feedback. Order IDs never repeat within a
controller. Append and replace validate their entire batch before mutation;
optional expected revisions reject stale tactical decisions. Paths, search
frontiers, and current sensor observations are transient.

The six orders are move, hold, follow, patrol, attack, and attack-move. Move and
attack-move specify a destination and arrival radius. Follow specifies a
friendly unit and separation. Patrol cycles through waypoints. Attack names a
hostile unit and may specify an engagement range. Hold and the continuously
maintained orders remain active until canceled. A failed order blocks the
controller until explicit retry or removal.

## Simulation boundaries

All decisions use detached own-unit data, acquired contacts, and fresh last
sightings. Terrain can inform routing; hidden units, mines, and undiscovered
conditions cannot inform tactics. Search is deterministic, resumable A* with
per-controller and global budgets. A route is a suggestion: the next segment
is rechecked against live terrain and the existing movement engine controls
heading, acceleration, collisions, and consequences.

Autopilot authority replaces only the requirement for a player in the cockpit.
It does not bypass startup, damage, heat, ammunition, target locks, arcs,
weapons hold, or other mechanical gates. A successful player control pauses
automation; an invalid request does not. The controller, unit effects, and
feedback share the normal world commit and rollback boundary.

## Parallel implementation ownership

Sol owns the public Lua contract, shared integration, tests, and final review.
Luna xhigh packet A owns typed orders and controller state; B owns actor
admission; C owns generic search; D owns filtered observations and pure combat
choice; E/G own steering and heartbeat; F owns persistence. Shared interfaces
must be agreed before dependent packets modify them. This separation keeps
navigation and tactics usable by a later battlefield director without giving
that director tick-level control.


## Tactical boundary

`btech.tactical.observe` accepts an explicit roster of 1–100 attached friendly
units on one map. Its version-1 detached snapshot includes each controller's
revision, permitted readiness, filtered intelligence, and bounded outcomes.
Aggregated contacts retain each observer and sighting time. Memory never adds
unobserved capabilities or confirms a hidden death. Raw controller sighting
caches are omitted from the tactical status; only filtered observations carry
remembered locations.

`btech.tactical.submit` stages a copy of every affected controller, validates
ordinary order admission and every expected revision, then publishes all copies
at once. A Lua `pcall` cannot turn a failed batch into a partial change. Shared
intelligence can guide movement, but attack orders still require the receiving
unit's own acquired hostile contact. Submission preserves explicit pauses and
does not change weapons policy.

The require-only `tactical_director` Lua package implements a replaceable pure
policy over snapshots. The opt-in encounter example invokes it on simulation
time and submits intentions through the same atomic boundary. It creates no
persistent groups or automatic global schedule. A future LLM can consume this
snapshot and produce intentions without access to tick-level movement or combat
execution.

## Time and rollback

An optional BattleTech simulation-clock extension persists elapsed simulation
seconds alongside the existing turn-phase clock. A committed heartbeat advances this counter once; loading never does.
Observations, sighting expiry, director cadence, and autopilot outcomes all use
this counter. Management operations stamp outcomes at their admission boundary.
The enclosing world transaction restores the counter with the controller and
unit effects on failure.

Search admission reserves up to one record per map cell for each active job,
within the global million-record limit. Additional jobs retain their intent and
wait; admitted searches keep their reservation until completion or invalidation.
This prevents global memory pressure from repeatedly destroying search progress.
Reduced-budget regressions exercise both one expansion per tick and capacity
for only one active frontier, and require every waiting route to finish planning.

Combat target selection uses staggered three-tick reassessment, preserving a
valid target between boundaries and immediately replacing invalid contacts.
The same hysteresis policy supplies attack-move diversion. Non-pursuing orders
need no pre-movement contact snapshot; their fresh post-movement observation
still updates sighting memory every enabled tick. Shot admission projects live
mode and damage heat before every mount and rechecks actual state afterward.
