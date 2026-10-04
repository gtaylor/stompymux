# Sprint implementation contract

Saved sprint state, admission, speed calculations and the movement acceptance
matrices below are implemented. Raw reference SQL unit import remains a separate
open dependency. This inventory follows the live reference callers and Rust
speed services.

## State and admission

`unit/mech_status_types.h` assigns sprint to secondary status bit 16 (0x10000).
`unit/mech_condition_state.c` reads and edits that bit. The reference can restore
it or receive it through administrative status edits. No ordinary sprint-enable
command or producer of EVENT_MOVEMODE was found in the inspected source. Do not
invent a transition timer or player command merely to make the mode reachable.
Rust now accepts bit 16 in `status_edits::secondary` and projects it through
`status_fields::secondary_status`. Both anatomy stores persist a typed boolean.

A typed unit-owned boolean must survive persistence and administrative edits for
both supported anatomy stores. Avoid another stored status word. Existing
transaction boundaries must restore state and output on failed edits.

`movement/mech_move_controls.c` clamps the requested speed before testing reverse
admission. The towing refusal runs first, with its SalvageTech exception; sprint
then rejects any remaining negative request with:

```
You can not backup while sprinting!
```

Rust's shared `motion_controls::require_reverse_allowed` now checks towing first
and sprint second. A missing tow no longer bypasses the sprint guard.

`combat/mech_physical_commands.c` rejects charge during sprint or evasion with
`You cannot charge while in a special movement mode!`. Its movement-mode event
check precedes that check. `movement/mech_ood.c` clears sprint only after orbital
drop admission succeeds. A rejected drop must retain the state.

## Shared speed calculation

The cache-return branch of `movement/mech_move.c::mech_cargo_maximum_speed`
applies these steps in order:

1. Begin with the load-adjusted cached maximum.
2. If any of MASC, supercharger or sprint is enabled, multiply two-thirds of
   that maximum by 1.5 plus 0.5 for each enabled boost.
3. With hot TSM, round the resulting walking MP using ties-to-even, add one,
   multiply by 1.5, round upward and convert back to KPH. While sprinting this
   step requires `btech_tsm_sprint_bonus`; without sprint it is unconditional.
4. Add one MP (10.75 KPH) for boolean Speed_Demon while sprinting and without
   EVENT_MOVEMODE. This is separate from its existing acceleration bonus.
5. Apply special-condition gravity using a minimum divisor of 50.

Do not multiply sprint after an already boosted maximum: simultaneous boosters
are additive in this expression. Preserve reference single-precision arithmetic
and the order of rounding. `btech_sprint_bth` has no production consumers in the
inspected reference; its configuration entry does not establish a to-hit rule.

Rust already has several intentionally distinct speed projections:

- `load::movement_maximum` applies external load to a damage-adjusted ceiling.
- `motion_controls::throttle_maximum` feeds controls and cockpit status.
- `motion` applies its movement ceiling during heartbeat advancement.
- `effective_speed` supplies building transfers, battle value and gunnery XP.
- Unit-local myomer/booster methods also feed damage, stand and stun handling.

The implementation must share the new bonus calculation while retaining those
callers' existing base-speed semantics. Thread the configured TSM sprint policy
through the host paths that already carry towing policy. A unit-local helper
cannot read the current pilot's Speed_Demon advantage; resolve that from World
at the shared world-aware boundary rather than saving another pilot flag.

## Required verification

Cover raw secondary-status import/edit/export, both anatomy stores, exact reverse
refusals and their ordering, charge admission and successful/rejected orbital
drops. Exercise native and Lua controls plus read-only status and persistence.

Speed cases need sprint alone, every MASC/supercharger combination, hot/cold TSM,
both TSM policy values, boolean advantage values 0/1/2, towing/cargo load, gravity
below/at/above the 50 floor, and rounding boundaries. Verify actual motion as well
as displayed ceilings, so a command-only change cannot pass. Compare building
transfer and XP consumers against the reference separately from throttle
admission. Existing non-sprint behavior must retain its regression coverage.


## Implementation checkpoint

`target/audit-sprint-state-tests.log` passes 15 tests across critical fields,
orbital insertion and sprint. The new sprint matrix covers all seven chassis:
native status editing, Lua rollback, persistence, reverse refusal for mobile
chassis, charge-selection refusal for Mechs, rejected orbital insertion retaining
the mode, and successful insertion clearing it. Library checking and formatting
pass. This does not establish sprint speed behavior, raw reference database
import, towing/sprint refusal interaction, or full admission precedence parity.

Next: share the reference bonus order across world-aware speed projections and
thread configured TSM sprint policy through the existing host callers. Add the
speed/advantage/load/gravity matrix and verify actual movement against readouts.


## Shared arithmetic checkpoint

`speed_bonus.rs` now owns additive MASC/supercharger/sprint arithmetic, hot-TSM
rounding, the one-MP Speed Demon bonus and the final gravity conversion. Effective
speed queries use the helper for both anatomy stores. The world-aware Mech path
reads the current pilot advantage, and jump load checking supplies that same live
input; no additional pilot flag is persisted. The unit-local query has no World
and therefore cannot include pilot advantages.

Two unit tests pass in `target/audit-sprint-speed-bonus.log`. They cover all eight
boost combinations, exact rounding ties, TSM policy behavior, advantage placement
before gravity, the gravity floor and invalid arithmetic inputs. The sprint
integration tests cover every supported chassis and advantage values 0/1/2,
read-only queries and persistence. After adding required character attributes to
the new test fixture, both sprint tests pass in
`target/audit-sprint-effective-speed-final.log`. The accompanying battle-value,
building-entry, jump and vehicle-XP targets passed 98 tests in
`target/audit-sprint-effective-speed.log`. Formatting passes.

This remains an intermediate implementation: configured TSM sprint policy still
needs to reach the world-aware callers (the effective Mech path currently uses
the enabled default), and throttle/actual-motion/turning projections have not yet
been connected. Vehicle environmental treatment still needs comparison at those
boundaries. Do not treat effective-speed tests as proof of actual sprint movement.


## Host-policy propagation checkpoint

A shared `SpeedPolicy` now carries towing and sprint myomer switches together.
Building entry, delayed entry rechecks, exit reconciliation and prone handling
use the live configured policy. The standalone effective-speed API retains its
explicit towing argument and standard sprint policy; its configured core accepts
both switches without duplicating the speed formula. The unit-local query and
jump-load caller currently use the standard enabled sprint policy.

The host building-entry matrix tests a hot sprinting Mech whose speed falls
between the enabled and disabled admission limits. It verifies acceptance only
with `tsm_sprint_bonus` enabled and no state changes after rejection. Existing
hot-TSM towing-policy checks remain in the same test. All 24 tests across
building-entry actions, prone and sprint pass in
`target/audit-sprint-host-policy.log`; library checking and formatting pass.

Outstanding: propagate policy through throttle, actual movement, turning, XP and
battle-value callers; connect the shared bonuses at those boundaries; update
speed validation envelopes and verify cross-caller agreement. Earlier default
policy limitations above are superseded only for the host paths named here.


## Throttle and live movement checkpoint

Configured native/Lua speed requests, Lua state readouts and cockpit status now
share a sprint-aware throttle function. Ground proposals, turning and VTOL
updates use the same world-aware bonus inputs, including current pilot advantage
and environmental gravity. Mech movement and turning retain their distinct
post-effective-speed conversions from the reference. `MovementRules` now
carries the configured TSM sprint switch from the server heartbeat.

Saved sprint motion limits cover the legal retained pilot/gravity/equipment
ceilings; invalid arithmetic yields a rejected non-finite bound. Vehicle
restoration reads sprint before validating throttle. The retained-control
checkpoint below supersedes this initial mode-dependent validation boundary.

All tests type-check. `target/audit-sprint-motion.log` passes 64 tests across
sprint, status, towing, VTOL controls and VTOL speed. The new end-to-end matrix
covers all five mobile ground chassis: native throttle, Lua readout, failed Lua
callback rollback, acceleration beyond the ordinary ceiling, validation at each
step and identical movement after saved replay. Formatting passes.

This does not close sprint: dedicated airborne VTOL/vertical-budget acceptance,
hot-TSM policy changes while moving, sprint clearing while above the ordinary
ceiling, prediction callers and remaining XP/battle-value policy propagation
need review and tests. The matrix must also cover simultaneous equipment boosts,
load and gravity through actual motion, rather than relying only on arithmetic
unit tests. The broad audit remains active.


## Retained controls and airborne replay checkpoint

Clearing the sprint bit at speed initially failed with `Invalid unit speed`.
Persisted motion now admits the envelope of previously legal pilot, gravity and
equipment settings independently of the current sprint bit. This is a structural
validation bound, not permission for a fresh command to use that speed. The
immobile-vehicle check continues to use actual propulsion capability.

The reference speed update begins with the retained desired speed and has no
blanket clamp to the current effective maximum. Its TSM-specific clamp remains a
separate rule. Clearing the administrative sprint bit consequently leaves both
actual and requested speed intact; ordinary controls and movement rules determine
subsequent motion.

The replay matrix now covers five mobile ground chassis and an airborne VTOL.
It clears sprint at speed, validates and saves the result, compares the following
movement tick with a restored world, then tests new controls against the reduced
ceiling and stops the unit. Direct ground requests above the new limit fail
without mutation; VTOL requests clamp to the remaining vector budget. Native
`speed run` uses the current ceiling for all six chassis. A separate airborne
case preserves a nonzero vertical command while accelerating horizontally past
the ordinary maximum, including validation and saved replay.

The original retained-control suite passed 52 integration tests in
`target/audit-sprint-retained-controls.log`, and all 239 library tests passed in
`target/audit-sprint-retained-unit.log`. With the airborne mode-clear and fresh
command checks added, all five sprint tests pass in
`target/audit-sprint-controls-final.log`. Formatting passes. These checks do not
replace full-suite acceptance of the current worktree.

Still open: hot-TSM policy changes while moving, combined boosters/load/gravity
through actual motion, tail-rotor damage while sprinting, prediction and remaining
XP/battle-value policy propagation, and raw reference database import of the mode.
The broader six-subsystem audit remains active.


## Experience and battle-value policy propagation

Classic gunnery difficulty and battle-value gunnery awards now receive both
`tsm_tow_bonus` and `tsm_sprint_bonus` from the shot policy. Mech and vehicle
firing adapters pass the same pair to their shared award context. The effective
speed calculation applies the selected policy to both participants. Battle-value
XP continues using nominal speed for its separate speed factors, as required by
that formula; the policy affects its live battle-value inputs.

The `bv` field, including Lua field inspection, now uses the host's configured
policy. Standalone battle-value queries use the standard sprint policy, while
the internal configured calculation is shared with XP. No second speed or
valuation formula was introduced.

The regression matrix uses hot sprinting TSM as attacker and target. Separate
base speeds cross classic XP and BV rounding boundaries: disabling the bonus
must equal the corresponding cold-unit calculation, and enabling it must alter
the applicable difficulty. It also checks both host field settings, Lua agreement,
read-only state and persistence with the pilot reconnected before XP replay.
Prediction/landing policy construction, jump-load queries, combined equipment
and environmental movement, and tail-rotor consequences still need review.

Verification: 78 focused integration tests passed across battle value, gunner
firing/skills, unit fields and vehicle experience. The first four targets passed
in `target/audit-sprint-xp-final.log`; after correcting the new test's rounding
boundaries and transient connection setup, all 12 experience tests passed in
`target/audit-sprint-xp-policy-replay.log`. All 239 library tests passed in
`target/audit-sprint-xp-unit.log`, all tests type-check, and formatting passes.
Earlier notes listing XP/BV policy propagation as outstanding are superseded for
the host paths named in this checkpoint. Full sprint acceptance remains open.


## Jump and predictive-fire policy propagation

Jump cargo admission now uses the configured hot-TSM sprint switch for both
projected and targeted jumps. The same launch function handles native commands
and Lua. Towing is rejected before this calculation, so the separate myomer
towing discount remains irrelevant there. Standalone tactical launch functions
retain standard policy.

Predictive firing now passes the host sprint switch into its shared motion
proposals. Manual landing and the server jump/landing heartbeat carry the switch
in their movement rules as well, preserving it for downstream consumers.

A cargo-heavy hot sprinting Mech now launches only with the bonus enabled. The
new test covers projected jumps and DFA, native/Lua agreement, no mutation on
rejection, failed-callback rollback after admission, and saved replay. A second
test predicts a hot sprinting target under each setting: the two settings select
different hexes, each predicted point matches live motion through shell travel
time, and native/Lua snipe queues the corresponding aim point. Prediction itself
is read-only and the Lua path starts from restored state.

This closes the known host policy omissions in jump-load admission and snipe.
It does not establish all sprint behavior: combined boosters/load/gravity in
actual movement, policy changes while already moving, tail-rotor consequences,
and raw reference database mode import remain to be addressed.

Verification: all 87 tests across jump, snipe, sprint and VTOL controls pass in
`target/audit-sprint-jump-prediction-regression.log`. The changed production
interfaces type-check across all tests (`target/audit-sprint-jump-check.log`),
and formatting passes. These results supersede earlier notes listing jump and
prediction policy propagation as outstanding; combined movement acceptance and
the broader six-subsystem audit remain open.


## Cargo and environmental gravity in live sprint

A new matrix reproduced a vehicle-only discrepancy: a sprinting tracked unit on
a special-rules map at gravity zero reported 71.666664 KPH effective speed,
while throttle correctly allowed 143.333328 KPH. The effective-speed vehicle
adapter applied bonuses but discarded map gravity. It now calls the same
world-aware bonus/environment helper as controls and movement, removing that
duplicated calculation. The reference's cargo-speed conversion applies gravity
after bonuses without a vehicle exemption.

The matrix covers all six mobile chassis, including an airborne VTOL, with zero
or seven half-ton cockpit crates. It checks ordinary maps with an ignored gravity
field, special maps at gravity 0/50/100/200, the gravity floor, one application of
the factor, read-only effective/throttle agreement, exact five-tick acceleration,
world validation and identical movement after save/load. Mech material mass is
fixed at nominal so construction-weight rounding does not obscure the gravity
comparison; quad acceleration retains its distinct divisor.

All 59 sprint, battle-value, vehicle-experience and towing tests pass in
`target/audit-sprint-gravity-final.log`. The initial failing characterization is
in `target/audit-sprint-gravity-before.log`. Formatting passes.

This covers cargo plus gravity during sprint, not simultaneous MASC/supercharger
or hot-TSM policy changes during movement. The effective-speed query now applies
gravity to vehicles even outside sprint; ordinary non-sprint throttle/proposal
paths still require a separate consistency review. Tail-rotor consequences and
raw reference database mode import also remain open.

The building-exit regression exposed an old vehicle-only exemption in its expected
result: on the fixture's special-rules 2g destination, vehicles must also use half
the effective ceiling. The expectation now checks that explicit factor, while
retaining heading, desired speed, flight and state-preservation assertions. This
is a consequence of the shared effective-speed fix rather than a separate exit
formula.

The corrected building-entry/exit and unit-field targets pass all 57 tests in
`target/audit-sprint-gravity-consumers-final.log`, giving 116 affected integration
tests passed across the two final runs. The first consumer run's outdated
expectation is recorded in `target/audit-sprint-gravity-consumers.log`.


## Ordinary-mode environmental consistency

The gravity matrix now also exercises sprint disabled. Its initial failure was
an ordinary Jenner reporting 236.5 KPH effective speed at the gravity floor while
throttle still allowed 118.25 KPH. Ordinary throttle, Mech/vehicle ground proposals,
turning and VTOL updates now apply the shared map-gravity step after their chassis
bonuses. Sprint uses that same environmental helper, so changing the mode no
longer changes whether map gravity is considered. Removed the unused turning
wrapper bypassed by this explicit conversion order.

Ordinary maps and special maps at gravity 100 preserve their incoming precision;
other special gravity values use the reference single-precision conversion and
50% floor. The expanded six-chassis matrix covers both modes, cargo 0/7, gravity
0/50/100/200, the special-rule gate, exact throttle/effective agreement, five-tick
acceleration, validation and saved replay. Its first draft unnecessarily applied
multiply/divide rounding at gravity 100; the expected value now honors the same
explicit bypass required by the reference.

This supersedes the previous checkpoint's ordinary-mode gravity omission.
Simultaneous installed boosters, hot-TSM policy changes at speed, tail-rotor
consequences and reference database import still need acceptance work. Existing
ordinary equipment conversions are preserved; this checkpoint does not claim
verification of every booster combination.

Verification: all 122 jump, snipe, sprint, towing and VTOL-control integration tests
pass in `target/audit-ordinary-gravity-regression.log`; all 239 library tests pass
in `target/audit-ordinary-gravity-unit.log`. Formatting passes. The initial
characterization is retained in `target/audit-ordinary-gravity-before.log`.


## Combined boosters and live hot-TSM policy changes

A live-motion matrix now covers all eight combinations of MASC, supercharger and
sprint, with zero/seven half-ton cockpit crates and gravity 50/100/200 (48 cases).
Equipment is installed in a valid Mech and activated through the booster controls.
The fixture's nominal material mass and exact cargo penalty isolate conversion
order. Effective/throttle values are compared against reference-order f32 bonuses;
ordinary f64 controls are allowed four relative f32 epsilon units. This is an
arithmetic precision allowance, not bit-for-bit numeric parity. The update's
separate one-booster multiplier or two-booster walking-MP conversion is checked
against each of five actual acceleration ticks. Requested controls, notices and
saved replay remain exact; every step validates the world.

A second test changes the host TSM sprint setting enabled/disabled/enabled while
a hot Blackjack is moving. Its Lua throttle readout changes between 96.75 and
86 KPH without modifying motion. Subsequent acceleration follows the distinct
118.25/96.75 KPH update ceilings, retaining the existing 96.75 KPH request. Each
policy stage validates and replays identically after save/load.

All 19 sprint and booster tests pass in
`target/audit-sprint-boosters-policy-regression.log`; formatting passes. The first
combined test used an absolute tolerance narrower than normal f32 rounding at
higher speeds; its final precision comparison is stated explicitly above.
Booster overload timers and failures retain the existing separate booster tests;
the combination matrix itself advances motion, not those timers.

This closes the listed combined MASC/supercharger/sprint/load/gravity and hot-TSM
policy-change movement cases. Tail-rotor consequences and raw reference database
mode import remain open. The wider six-subsystem audit is still active.


## Tail-rotor damage uses the world velocity budget

The first tail-rotor critical used the chassis-only maximum when limiting desired
speed. A loaded sprinting Kestrel at low gravity therefore requested 128.856926
KPH after damage instead of the reference-order 336.044097 KPH limit. Rotor damage
now enters a shared world transition, deriving the current loaded/mode/gravity
ceiling before mutation and reserving the vertical component before applying
cruise minus 0.1 KPH. Actual momentum is preserved. Repeated tail-rotor damage
reports the repeated hit without applying another control limit, even when the
map environment has changed in the meantime.

The transition is used by the material hit and selected-critical paths. Inventory
correction: ordinary/FASA hit tables select main-rotor damage or destruction,
not tail-rotor damage; advanced tail damage is reached through selected criticals.
There was no reachable ordinary-hit-table tail bypass. The common handler keeps
material and control consequences together without introducing another formula.
First and repeated tail-hit messages now match the reference, including the
first-hit warning style. The reference comment mentions slower turning, but its
current movement code has no tail-rotor flag consumer implementing a separate
turn-rate change; no additional turn penalty was invented.

The new live test covers first-hit control limiting with sprint, cargo, gravity
and vertical speed, preserved actual speed, fresh throttle refusal/acceptance,
saved critical replay and repeated damage after an environment change. All 40
sprint, vehicle-impact, VTOL-critical/impact/control/speed tests pass in
`target/audit-sprint-tail-final.log`. The original ceiling mismatch is recorded in
`target/audit-sprint-tail-before.log`. Earlier attempts to select tail damage from
the ordinary hit table were corrected after inspecting its actual outcomes.

Raw reference database import of the sprint bit remains open; the broader audit
is not complete.

All 239 library tests also pass in `target/audit-sprint-tail-unit.log`; formatting
passes and the reference tree remains unchanged.


## Reference import dependency

Current persistence reads complete Rust-owned `btech_units`/vehicle records; it
does not reconstruct units from `btech_mech_runtime` and the reference construction
and section tables. The raw `status2` sprint bit therefore cannot be imported in
isolation without a complete reference-unit importer and a defined ownership rule
when both formats are present. No partial overlay or compatibility shim was added.
The typed sprint boolean is covered by Rust persistence and administrative bit-16
edits. Raw SQL unit import remains open with the larger unit persistence work;
it is not an unimplemented ordinary movement command.
