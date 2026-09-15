# Porting audit implementation progress

Tracks the six sections requested from [the porting audit](claude-porting-audit.md).
The audit remains the requirements inventory; this ledger does not replace or
reduce its scope. Items are complete only after implementation and relevant
verification. The reference tree is read-only.

## Combat

Implemented and verified for existing above-water firing: configured
range-dependent energy damage is now supplied to the
shared packet calculation for Mech, vehicle and terrain attacks. It adds one at
range <= 1, subtracts one beyond medium, and halves beyond long, after focusing
damage and before glancing reduction. Ballistic/missile profiles are unaffected.
Boundary tests and all four Mech/vehicle shooter-target combinations pass through
native and Lua firing.
Underwater profiles remain outstanding and must supply their own range limits
when underwater firing is implemented.

Implemented: `divrotordamage` now reaches the common vehicle exterior-damage
resolver. Positive rotor hits are divided with integer truncation and a one-point
minimum, before hardened armor. Zero damage, non-rotor sections and direct internal
damage retain their behavior. Configuration follows nested impact policy without
changing persisted unit state. Tests cover disabled/unit/large divisors, rounding,
zero damage, hull/internal exclusions and persistence replay. Clippy and 228 tests
passed (205 library plus armor, VTOL hit, vehicle salvo and orbital-drop combat).

Implemented airborne aim terms in the shared target calculation: moving VTOL
+1 (including reverse and vertical motion), LBX cluster -3 versus any VTOL, and
Stinger -3 versus flying rotorcraft / -1 during orbital descent. A jumping Mech
receives no extra Stinger bonus. The orbital term remains while jump jets
compensate for a breached cocoon; the reference's out-of-control predicate means
nonzero cocoon state, including that continuation. Stinger target admission also
accepts ground vehicles in orbital descent. Tests cover all seven supported
shooter shapes, actual firing, read-only previews and persistence replay.
Validation: 407 broader aim/firing/movement/orbital-drop tests passed, followed
by 218 library and focused tests after the final orbital-target admission fix.
Clippy with warnings denied and formatting checks passed. This is targeted
verification, not a new full-suite run.

Implemented the attacker-in-water +1 for unit-target aim, shared across shooter
classes and exposed as `attacker_water`. Signed elevation distinguishes submerged
chassis from surface hovercraft and aircraft. Tests cover all supported chassis,
zero-depth water, shallow water, target-only water, read-only previews and actual
Mech firing. The library/movement/terrain-fire regression run passed 576 tests.
Fully submerged weapon profiles and admission remain outstanding.

Occupied-woods accuracy now contributes -1 for light woods or -2 for heavy woods
when `moddamagewithwoods` is nonzero. A shared target query uses underlying terrain
and signed integer elevation, including the two-level canopy boundary, for Mechs,
quads, vehicles and VTOLs. This remains a separate signed subtotal term and is
zero for coordinate-only aim. Smoke/fire overlays do not erase the base forest.
Native and Lua sighting use the same configured rule. Tests cover all 49 supported
shooter/target chassis pairs, enabled/disabled settings, terrain, altitude,
overlays, read-only previews and restart. All 234 selected library, accuracy,
digging and sighting tests passed across `target/audit-woods-accuracy-tests.log`,
`target/audit-woods-sighting-tests.log` and `target/audit-woods-config-tests.log`.
The latter supersedes the configuration-fixture failure in the sighting log.

Single-hit armor damage now shares occupied-woods absorption across Mech and
vehicle targets and both firing adapters. It subtracts 2/4 with a minimum of one
before glancing rounding, applies intentional woodland effects using the original
damage and publishes both cockpit absorption messages plus terrain notices.
The report retains pre/post absorption damage and the woodland result; terrain,
damage, dice and messages obey the enclosing action/callback rollback.
Tests cover all 49 supported shooter/target pairs, native/Lua agreement,
configuration off/on, restart, light/heavy/absent woods, energy/ballistic damage,
glancing, the damage floor and callback failure. A targeted clearing roll proves
the terrain resolver receives ten damage while only six reaches the target.
All 289 selected library/woods/terrain/salvo/vehicle-fire/terrain-fire/swarm tests
passed in `target/audit-woods-single-tests.log`,
`target/audit-woods-single-final-tests.log` and
`target/audit-woods-damage-final.log`; the last supersedes the first run's missing
Gauss-ammunition fixture failure.

Missile armor damage now uses shared packet absorption after cluster adjustment,
the swarm incoming cap and AMS interception. It subtracts 2/4 from total damage,
rounds down to whole missiles and may stop the salvo completely. Pre-absorption
damage reaches the same transactional woodland resolver used by single hits.
Feedback distinguishes one missile, all missiles and some missiles. Inferno
keeps its separate burning path. The pre-defense hit count is unchanged, so
missiles absorbed by woods do not return to a continuing swarm flight.

All 351 selected library, woods, swarm, scan, building-recovery, vehicle-fire,
salvo and MML tests pass in `target/audit-woods-missile-complete-tests.log`.
The missile matrix covers all 49 supported source/recipient pairs with the
option off/on, native/Lua state and feedback agreement, and restart. Additional
tests cover live IS/Clan AMS with partial/complete interception, both MML payload
families, single-missile feedback, Inferno bypass, zero-damage hit-location dice,
callback rollback, and pre-cover missile expenditure across swarm hops. The
packet-level matrix also covers glancing cluster adjustment and payload rounding.
Early test failures came from point-blank LRM minimum-range penalties and fixture
relocation admission; the final run uses normal shutdown/placement/pilot setup
at six hexes. No production admission rules were relaxed.
Clippy checked all test targets with warnings denied in
`target/audit-woods-missile-clippy.log`. Formatting and whitespace checks pass;
the reference tree is unchanged. This is focused acceptance, not a completed
full-suite run or completion of the overall audit.

**The woods-damage option is not complete:** pellet/burst absorption,
thermal-only attacks and explicit hex-mode interactions still need implementation
or characterization. Single-hit resolution subtracts 2/4 with a minimum of one damage;
missile/pellet/burst resolution subtracts from total damage, truncates back to a
whole projectile count and may absorb everything. Both invoke intentional
woodland effects with pre-absorption damage; missile feedback differs from
single-hit feedback. Missile integration now covers interception, glancing,
swarm continuation and existing transactional terrain effects. Pellet and burst
weapons still need their own verified ordering, not a blanket per-packet
subtraction. The reference also calls its ordinary damage calculation
before entering the non-missile multi-hit path; audit that ordering for LBX and
burst weapons rather than assuming they follow the missile-only path exactly.

Still outstanding: woods absorption
and feedback, underwater firing,
autoejection and MechWarrior construction. Deferred
class-specific combat remains identified by the source audit.

Sixth-sense warnings now use one service for all supported unit anatomies.
Startup completion captures the player's Sixth_Sense advantage; changing it
while running does not resample the cache. An explicit unit lock rolls 2d6;
8 or less schedules a private warning after a further 1–3 second draw. Range
bands (<9, <20, otherwise) and signed current-mass differences (<=-20, >=20
tons) select the nine reference messages. Observer locks skip the roll, while
coordinate locks and lock clearing skip the service. Gunner locks use the
physical parent's dice and preserve the pilot's independent selection.

Independent pending warnings live in each recipient's validated saved state.
They keep idle worlds ticking and survive restart. Delivery checks the current
pilot rather than capturing a player at lock time; disconnected or unconscious
pilots discard the due event. Private notification and event consumption share
the server's commit boundary. Tests exercise all 49 source/target chassis pairs,
startup snapshots, delay draws, concurrent events, native/Lua callback rollback,
gunner selection and pilot-only output. Server fault injection covers both unit
storage tables with a due event on a shut-down battlefield: failed commits
retain the event and publish no warning, and retry delivers after a successful
save. Custom template SS_Ability import is not implemented by this service.
The 244 library, sixth-sense, startup and targeting tests pass in
`target/audit-sixth-sense-acceptance.log`, including the live server fault tests.
All-target Clippy with warnings denied, formatting and whitespace checks pass;
the reference tree remains unchanged.
This is focused acceptance, not a new full-suite result or completion of the
overall audit.

Verified audit discrepancy: woods clearing already matches the reference's
no-op mine-removal helper (`combat/mine.c::mine_field_possibly_remove`). The audit
now records this distinction; implementing mine removal would be a gameplay
change rather than filling a behavioral porting gap.

Validation: 204 library tests, 37 vehicle-firing tests and 37 terrain/salvo/MML/
Swarm/weapon-damage regression tests passed (278 total). `cargo check --all-targets`,
Clippy with warnings denied, formatting and whitespace checks passed. The full
suite has not been rerun for this audit change.

## Movement

Periodic piloting characterization preceded the implementation below. Read-only tracing of
`movement/mech_update_heartbeat.c`, `movement/mech_update_piloting.c` and
`unit/mech_identity.c` establishes these requirements for the hook:

- Only started units or unconscious pilots enter this heartbeat path.
- Odd event ticks round up before `% TURN`, admitting both 29 and 30.
- Shutdown sets modifier +3 on that boundary, but the shared check fails without
  rolling. Conscious shut-down units never reach it through this heartbeat.
- Special conditions plus nonstandard gravity and speed above the unloaded
  maximum select internal leg damage on failure. Quads visit arms before legs;
  already-destroyed sections are skipped. The condition is not an explicit
  gravity-below-100 test.
- Damaged-gyro/hip running checks execute every heartbeat outside the boundary,
  fallen, jumping and out-of-control guards. Their modifier is zero; failed
  ordinary checks delegate a level-one fall. The shared prone exception still
  makes already-fallen Mechs succeed without rolling.
- TSM changes the local running threshold only inside the guarded turn branch.
  A per-unit startup-relative timer would not reproduce this global phase.

The shared piloting service had a prerequisite gate bug: it checked only an
assigned player's recovery, ignoring unit-owned tactical recovery and blindness.
Mech and vehicle adapters now share the same power/recovery/blindness predicate.
Blocked checks consume no dice; Mechs preserve the reference's prior prone
automatic-success rule, and vehicle destruction still blocks vehicle rolls.
`tests/btech_piloting_admission.rs` exercises both blocked conditions alone and
together on all seven chassis, real tactical injuries, restart, ordinary
recovery/flash ticks, and resumed rolls. This fixes existing control checks; it
did not verify the subsequently implemented periodic scheduler or gravity damage.
All 234 library, shared admission, crew, empty-crew, prone and vehicle-piloting
tests pass in `target/audit-piloting-admission-acceptance.log`. These tests verify
the prerequisite fix and existing consequences, not the subsequent heartbeat hook.
All-target Clippy with warnings denied, formatting and whitespace checks pass;
the reference tree remains unchanged.

The periodic hook now uses a committed global phase with the characterized
29/30 cadence. Gyro/hip running checks remain outside the turn guards; gravity
stress invokes the existing internal-damage pipeline in reference leg order.
Ordinary failures use the shared Mech/vehicle fall services. The action stages
notices, XP and casualties with the host transaction, and a failed save restores
both the phase and all consequences. Idle worlds still save one clock update
per second; no offline elapsed time is simulated on load.

Acceptance now passes seven periodic integration tests and the library tests in
`target/audit-periodic-acceptance.log`. Coverage includes real hip and gyro
criticals, reverse/walk/running boundaries, TSM's turn-only threshold, gravity
success/special-rule gates, ordered biped/quad internal damage, all supported
chassis' shutdown/unconscious gates, persisted replay, active fall commit failure,
and otherwise-idle clock rollback, wrap and restart after a wall-clock jump.
The idle fixture explicitly expires the independent reactor startup grace.
Generic private piloting-roll notices, broader damage-cascade edge acceptance,
and computer-failure processing remain open; this is not full movement parity.

All 357 movement tests pass in `target/audit-periodic-movement-regressions.log`.
Two speed assertions now expect the reference integer message. Three server
retry tests explicitly expect phase one after their successful retry, retaining
whole-state phase-zero assertions after rejected saves. Comparing the previous
failure output established that the turn phase was their only state difference;
no damage, dice, heat or rollback assertion was removed.
The combined acceptance is 579 tests (215 library, 357 movement, seven periodic).
All-target Clippy passes with warnings denied in `target/audit-periodic-clippy.log`;
formatting and whitespace checks pass. The reference tree remains unchanged.
The earlier full-suite stop is resolved for these five tests, but a complete
full-suite pass is not claimed.

Action-time stagger tracing found a separate scalar contract. Reference
`mech_condition_state.c`, `mech_stagger_level` and `mech_stagger_modifier` read
`rd.stagger_damage`; the live rolling history does not update it. Source writers
only reset it to zero or set -10. The script value is read-only, but SQLite
runtime restoration accepts a positive scalar. Therefore ordinary damage-history
sums must not be substituted for the action-time trigger.

Rust-owned saved stagger state now retains a separate signed `action_damage`
scalar, defaulting to zero. It feeds read-only `StaggerDamage` inspection and the
STAGGERING cockpit banner. Controlled drops use positive twenty-point levels as
an additional piloting modifier and require at least a level-one fall check,
even while stopped. Speed notices precede the stagger warning. Shared roll/fall,
XP, flooding, mine and transaction services still own all consequences; no
parallel drop resolver was added. Clearing the rolling damage window does not
clear this scalar, matching the reference's separate storage.

The biped/quad acceptance matrix covers -10/0/19/20/40/60, stopped/fast-walk/run,
success/failure, recent history that must not activate the scalar, read-only
field enforcement, status banners, saved replay and complete Lua rollback.
All 61 prone/status/unit-field tests pass in
`target/audit-action-stagger-acceptance.log`. Original C runtime snapshot import,
jump-admission scalar consumption and complete reset lifecycle acceptance remain
open. Very large scalars use the shared piloting modifier ceiling; exact
out-of-range diagnostic fidelity remains follow-up rather than a claim of
unbounded numeric parity.
Validation also passes all 215 library tests, seven periodic-piloting tests,
one fall-heading test and ten existing movement stagger tests, for 294 distinct
selected tests total. Shared-service results are in
`target/audit-action-stagger-shared.log` and
`target/audit-action-stagger-regressions.log`; the filtered periodic invocation
selected no tests, so its subsequent unfiltered run supplies that coverage.
All-target Clippy passes with warnings denied in
`target/audit-action-stagger-clippy.log`. Formatting and whitespace checks pass;
Lua annotations and operator help reflect the restored scalar. The reference
tree remains unchanged. No complete full-suite pass is claimed.

Action-time landing now consumes the separate scalar. It snapshots the
staggering condition before DFA, resolves the normal surface transition, then
checks before ordinary damaged-leg/gyro rolls. The modifier is scalar level plus
weight class, with tonnage always applied for action-time checks; the existing
rolling check still honors its tonnage setting. One weight helper serves both
policies. Failure delegates the level-one fall and its ordinary consequences,
retains jump stabilization, and returns before further normal landing work.
The action path uses existing piloting XP and fall publication/rollback services.

`tests/btech_stagger_landing.rs` verifies all four weight classes (including a
quad), scalar 0/19/20/40, roll outcomes, warning order, unchanged dice when no
check is due, one control roll on success, stabilization and saved replay.
A damaged-gyro case verifies that stagger failure skips the ordinary gyro roll,
while success continues to it. Both tests pass in
`target/audit-stagger-landing-acceptance.log`. Jump admission and broader reset
lifecycle/C-runtime import requirements above remain open.
All 300 library, jump, crew-landing, heading, reassignment, periodic-piloting,
prone and stagger-landing tests pass in `target/audit-stagger-landing-regressions.log`.
All-target Clippy passes with warnings denied in
`target/audit-stagger-landing-clippy.log`; formatting and whitespace checks pass.
The reference tree remains unchanged. This is focused acceptance, not a new
complete full-suite run or closure of the wider audit.

Validated pre-launch attempts now check action-time stagger using the same
modifier as landing. The launch path retains the scalar/history distinction,
warns before the roll, and returns a completed attempted action with a shared
level-one fall on failure. It does not create flight/last-jump state or engage
stabilization for a launch that never happened. Projected and DFA host adapters
share one action wrapper for notices, control XP, private injury/fall consequences
and casualty publication. Error paths restore world and effect checkpoints;
low-level tactical callers use a candidate world. Native dispatch no longer
maintains a duplicate publication/rollback implementation.

`tests/btech_stagger_launch.rs` verifies biped/quad, projected/DFA, scalar
0/19/20/40, success/failure, history-only bypass, exact native/Lua agreement,
no launch on failure, dice preservation on bypass, one roll on success,
restart and whole-callback rollback. Lua's boolean denotes an accepted attempt,
which can finish in a stagger fall rather than a flight; annotations now say so.
The matrix passes in `target/audit-stagger-launch-acceptance.log`.

Launch rejection ordering is now addressed within the existing command syntax.
Native argument text and typed Lua requests enter shared admission unchanged.
After admission and any stagger roll, one read-only preparation step resolves
arguments, default targets and route geometry. A failed roll never interprets
the destination. A successful roll followed by rejection commits its dice/XP
and sends the rejection only to the pilot; the overall host action still rolls
back on a save/publication or enclosing Lua failure. Low-level tactical callers
receive a completed attempt with rejection notice, consistent with attempts that
finish in a fall. Battlefield/underground admission remains before the roll.

Two additional tests cover malformed syntax, invalid bearing, range, explicit
and default targets, passenger/private delivery, successful-roll persistence,
failed-roll short-circuiting and Lua rollback. Early underground refusal is
verified to leave the full BattleTech state unchanged. All three launch tests
pass in `target/audit-jump-order-acceptance.log`.
Character casualty edge cases, wider command syntax/text parity and full scalar
reset/C-runtime import coverage remain open.

The updated ordering passes all 296 selected library, jump, crew-landing,
heading, reassignment, prone, stagger-landing and stagger-launch tests in
`target/audit-jump-order-regressions.log`. This is focused acceptance rather
than a full-suite pass. All-target Clippy passes with warnings denied in
`target/audit-jump-order-clippy.log`; formatting and whitespace checks pass.
The reference tree remains unchanged.

Earlier launch-path acceptance passed all 294 library, jump, crew-landing, heading, reassignment, prone, stagger-landing
and stagger-launch tests pass in `target/audit-stagger-launch-regressions.log`.
All-target Clippy passes with warnings denied in
`target/audit-stagger-launch-clippy.log`; formatting and whitespace checks pass.
The reference tree remains unchanged. The broader audit and full-suite run
remain open.

Scalar lifecycle acceptance found one missing normal-landing reset.
`mech_jump_land` ends by stopping the scalar stagger check only in traditional
mode. Its earlier stagger-failure return bypasses that reset. Rust now preserves
this distinction: normal traditional completion zeros `action_damage`; rolling
modes and the early stagger-failure path retain it. The independent rolling
history is not erased by this scalar reset.

The landing matrix covers negative/below-threshold/active scalars, successful
and failed rolls, all three modes, both Mech anatomies and saved replay before
and after landing. Further tests verify that reference-style map reassignment
preserves the full stagger state even when resetting out-of-bounds coordinates
to the new origin, and that core material destruction clears the scalar.
`scenario_map` already implements the preserving path; Rust administrative
placement is a different operation and was not changed by this audit step.
The remaining restored scalar-event scheduler/import and wider lifecycle gaps
are not closed by these tests.

During that trace, conventional landing incorrectly classified every unassigned
cockpit as unconscious. It now uses shared `crew::unit_unconscious` and the
reference unconscious-landing text, suppressing the normal completion message
for that failure. The biped/quad acceptance matrix covers assigned/unassigned
crews, real injuries, all flight ticks, material posture, exact messages and
saved-state replay in `tests/btech_jump_crew.rs`. Reference sources were read only.
All 296 library, jump, heading, pilot-reassignment, empty-crew, control-admission
and prone tests pass in `target/audit-jump-crew-acceptance.log`. Clippy checks all
targets with warnings denied in `target/audit-jump-crew-clippy.log`; formatting and
whitespace checks pass. The attempted `btech_motion jump` filter selected zero
tests and is not counted as evidence; the dedicated jump suites supply coverage.
This does not close action-time stagger or the broader movement audit.

Implemented shared reverse-towing admission for Mechs and vehicles, retaining
the reference SalvageTech exception and exact refusal text. Stopping, forward
throttle and reversing without a tow retain their existing rules. The named
speed parser, direct controls and Lua reach the same check before mutation.

Jump and DFA admission now reject a reduction greater than one MP between the
damage-adjusted unloaded maximum and effective loaded speed. They reuse the
existing mass, CargoTech, booster, TSM and special-gravity calculation; exactly
one MP remains legal. No second cargo calculation or persisted load cache was
introduced. Boundary tests exercise projected/targeted launch, native/Lua
agreement, refusal atomicity and restart. Two overweight test retrofits now use
explicit nominal test mass to isolate their gyro/jet scenarios.

The timed iNARC pod-removal check is still open: reference command mask 18 assigns
REMOVEPODS to ground vehicles and VTOLs, while Mech REMOVEPOD is an immediate
swat. Rust preserves that split and has no Mech removal timer. This jump guard
depends on implementing ground-vehicle jumping, rather than inventing a Mech
cooldown. The audit requirement is retained.

Verification passes 682 library, cargo, jump, movement and towing tests. The
towing matrix covers six mobile chassis, native/Lua/direct requests, the
SalvageTech exception, forward/stop controls, detached reverse and restart.
The full suite has not been rerun for these admission changes.

Cable-command prerequisite verified: reference ATTACHCABLES and DETACHCABLES
carry mask 128 (`GFLAG_MW`), and registry dispatch maps that flag exclusively to
`CLASS_MW`. They are actions performed by an ejected MechWarrior on other units,
not additional cockpit pickup controls. Their implementation remains required
alongside the missing MW class; no replacement command with different operator
authority has been introduced.

Water speed audit discrepancy: `mech_update_speed.c::mech_terrain_speed` uses
`speed * MP1 / (MP1 + penalty)`, so its intended shallow/deep reductions are
one-half and one-quarter speed. The live caller passes raw map depth through
`mech_position_elevation` -> `mech_hex_elevation_get` -> `map_elevation_get`.
`map_coding_get_index` rejects negative elevations and the decoder returns the
stored value unchanged. The negative-depth branches cannot run for normal maps.
The separately named `battle_map_hex_elevation` performs the signed conversion,
but this caller does not use it. A user choice is pending between enabling that
intended slowdown and preserving executable reference behavior. Existing entry
checks and water running limits remain in place; no depth-speed change was made.

Speed confirmations now share one formatter across Mech and vehicle controls,
including VTOL horizontal throttle. They use the reference text
`Desired speed changed to N KPH.` with truncation toward zero. Accepted motion
retains its full precision. Existing native/Lua cockpit and TCP tests have been
updated to assert the reference text, including fractional speed and stopping.
All 283 selected library, power/TCP, towing, vehicle-driving and movement-control
tests pass in `target/audit-speed-feedback-tests.log` and
`target/audit-speed-feedback-controls.log`. The control test also verifies
negative fractional requests, native/Lua notice routing, full-precision saved
motion, restart and transactional replay. Other movement-message differences
remain in the audit; this changes only successful speed confirmations.
Clippy checked all test targets with warnings denied in
`target/audit-speed-feedback-clippy.log`. Formatting and whitespace checks pass;
the reference tree remains unchanged. The full audit and full-suite run remain open.

Not yet addressed in this audit pass: generic piloting-roll notices, water-speed choice,
sprint, action-time stagger, vehicle jump/pod admission, VTOL ground and landing
rules, cables, skid and movement messages.

## Sensors

SENSOR output now comes from a shared read-only renderer. Matching slots use a
single `Sensors:` line; mixed slots use the Sensors header, seven-dash underline
and aligned Primary/Secondary rows. All nine modes carry the reference names,
range text, blocking text and notes. Visual and amplification modes show
360-degree scanning when the slots match or the vehicle is stationary, otherwise
120-degree forward scanning. Other modes omit arc text. Any single command
argument enables verbose active descriptions; Wanted remains compact and no
longer adds a non-reference seconds-remaining suffix. `btech.unit.sensor_report`
uses the same renderer. Switching countdowns and their Lua state remain owned by
the existing selection service. MOVE_NONE Mech representation remains an open
construction requirement, so this renderer does not invent fixed Mech state.

New acceptance checks cover compact names/ranges for all nine modes and exact
mixed/verbose/Wanted output across all seven chassis, including fixed-platform
arcs, arbitrary one-argument inspection, native/Lua agreement, restart and
complete unit-state immutability. Existing command tests now expect the
reference layout while retaining countdown and callback-rollback assertions.
The 226 library and sensor/report/light/chassis tests pass in
`target/audit-sensor-report-acceptance.log`; nine additional movement-side sensor
tests pass in `target/audit-sensor-report-motion.log`. That broader run corrected
one remaining assertion that automatic daylight fallback clears a lock. It now
checks the exact retained selection and visual fallback, while still proving
that completed deliberate switches, shutdown and explicit clear remove locks.
These are focused results, not full-suite completion.
Final native/Lua report verification passes after applying the standard Lua
error wrapper. All-target Clippy with warnings denied, formatting and whitespace
checks pass; the reference tree remains unchanged.

Map-light rechecks now use shared slot-transition logic and return cockpit
notices through native and Lua editing transactions. A changed day setting
replaces light-amplification in each distinct active mode and emits the exact
reference bright-light warning for running, surviving units, in map membership
order. It preserves target locks and pending sensor changes. Identical primary
and secondary modes are checked once, leaving the secondary unchanged: L/L
becomes V/L. Reapplying the same map light, including visibility-only edits,
does not recheck. Stopped units change slots silently. These details follow
`sensors/mech_sensor_selection.c::sensor_light_availability_check` and
`map/map_conditions.c::battle_map_light_set`; the reference setter performs no
implicit lock clear. No supported sensor has a positive minimum-light value,
so the audit's dark-warning example is unreachable with valid light 0–2.

`tests/btech_sensor_light.rs` covers all seven chassis, four slot combinations,
running/stopped states, exact messages, unchanged-light edits, preservation of
all other unit state, restart, cockpit audience, both Lua map APIs, native
map conditions and callback rollback. The normal deliberate sensor-switch
countdown remains a separate operation.

The broader sensor run exposed a stale stationary-radar expectation at 200
hexes in `tests/btech_scanner_chassis.rs`. The reference's LOS gate caps radar
endpoints at 180 before tracing, regardless of a stationary sensor's hardware
range bonus. The test now checks both mobile and fixed radar at 179 (visible)
and 200 (blocked), retaining the existing stationary probe-extension cases.
No radar production rule was relaxed to satisfy that assertion.
All 237 library, light-change, map-field, mixed/chassis/vehicle sensor,
LOS-ceiling and help tests pass in `target/audit-sensor-light-acceptance.log`.
All-target Clippy with warnings denied, formatting and whitespace checks pass;
the reference tree is unchanged.
This is focused acceptance; the entire audit and full-suite verification remain
open.

Unit markings now use one validated text type (16,383-byte bound) in both unit
records. Cockpit `view [contact]` and Lua `unit.view` share running admission,
the operator's own target selection, current acquired-contact visibility and
unblocked terrain LOS. There is no detailed scan-range gate or contact/dice
mutation. Missing and invisible-target feedback follows the reference, including
its default-target spelling. Ordinary lifecycle pruning already clears removed
targets; viewing does not introduce a second target-cache lifecycle.
Lua `unit.markings` reads raw configuration; wizard-only `unit.set_markings`
validates before writing and participates in callback rollback. Display escapes
literal markup. Full Rust unit records persist markings without a second shadow
configuration store; this does not implement reference-configuration import.

Native VIEW now dispatches by location: cockpit/gunner viewing is public through
normal control checks, while map-room `VIEW X Y` retains its wizard check.
The independent C access fixture remains unchanged; its comparison explicitly
accounts for this shared command name. Help VIEW now describes both contexts.
All 226 library, access, markings, map-view and gunner-report tests pass in
`target/audit-markings-acceptance.log`. Coverage includes all 49 source/recipient
pairs, native/Lua agreement, restart, empty/escaped text, byte-bound validation,
hidden targets, scan-range zero, administrative denial, callback rollback and
an ordinary gunner's own selected target plus map-view denial.
All seven help tests also pass in `target/audit-markings-help.log`, including
the supplied corpus index. Clippy checked every test target with warnings denied
in `target/audit-markings-clippy.log`; the initial test-module ordering warning
was fixed without suppressing it. Formatting and whitespace checks pass, and
the reference tree remains unchanged. Full-suite completion remains unverified.

Implemented radar's flying-type -3 aim bonus below altitude ten, without
relaxing terrain/altitude/clearance/range detection gates. Live VTOL and restart
tests exercise the target-type adapter. Dug-in mobile vehicles now use z + 0.1
sight height; fixed installations retain z + 1.5, matching reference precedence.
A ridge test verifies both sight directions, read-only queries and restart.

Verified another audit discrepancy: active probes already use the shared 140%
fixed-installation range. Tests now verify 8/4/11-hex boundaries for all three
probe families and prove that stopping a mobile vehicle does not grant the bonus.
The `MOVE_NONE` Mech representation remains missing and is not marked complete.

Implemented the worst-case LOS distance cutoff before terrain tracing and the
high-altitude shortcut. Unit endpoints share spatial range calculation: either
endpoint's AntiAircraft installation selects 180 hexes; otherwise the map's
maximum visibility applies. Equality is permitted. Coordinate targets use the
same rule with only the observer's installation, and retain their computed range
even when blocked. This does not grant sensor acquisition outside a sensor's own
limits or bypass terrain obstruction. Tests cover seven chassis, both sight
directions, fractional boundaries, vertical separation, live map changes,
read-only queries and restart, plus coordinate limits on both smaller and larger
maps.

LOS-cutoff verification passed 394 library, range, scan, radar, vehicle LOS,
coordinate-fire, firing, artillery, network, probe and searchlight tests.
Formatting, whitespace checks and Clippy with warnings denied passed. This is
focused verification; the full suite has not been rerun for the cutoff change.

Still outstanding: personal ECM, artillery observer datalink,
stationary Mech representation, detection/perception XP,
SCAN/REPORT/C3TARGETS formatting and TAG messages.
The audit's class-specific detection requirements remain recorded.
Focused/library verification passed 228 tests. Broader verification passed 469
aim, movement, scan, firing and vehicle-sensor tests after correcting one stale
status-selector assertion (the failing motion test was rerun separately). Clippy
with warnings denied passed. The complete six-section audit remains open.

## Map and special objects

Implemented load recovery for surviving damaged building interiors referenced
by entrances. Missing clocks resume at 120 seconds, saved clocks retain their
remaining time, and unrelated maps do not gain repair events. The same building
state helper chooses ordinary repair intervals after damage and repair ticks.
Loading remains read-only and does not advance offline time.

The countdown writer compares actual stored rows, so an inferred clock is
inserted on its first save even when the loaded world has not otherwise changed.
Updates preserve extension columns; failed inserts roll back. Tests cover absent
and empty timer tables, duplicate entrances, partial intervals, completion,
restart, extension data, rejected saves and idle-server startup. All 393 selected
building, map, movement and heartbeat tests passed, along with formatting and
Clippy. Full-suite verification remains outstanding.

The two-hour rebuild is still open pending the behavior choice: the reference's
`possibly_start_building_regen` returns when `get_building_cf` returns zero,
making its later zero-integrity rebuild branch unreachable. A functioning
7200-second rebuild would correct that bug rather than reproduce executable
reference behavior. The existing 120-second storage constraint is unchanged.

Not yet addressed in this audit pass: building rebuild and step
notices, tolerant map parsing, map validation replies,
special-object registration/admission/help, LIST/FIXMAP/SETCOND output and map
transition messages.

Authored eternal fires now infer map flag 8 before any explicit metadata override.
Temporary-fire and smoke asset aliases still normalize to grassland. The
stale-fire save fixture explicitly clears flag 8; native/Lua loading, export,
save preparation, publication and restart verify permanent fire survives.
The gunner structure-scan restart test now expects the independently recovered
120-second building timer while checking the remainder of the world unchanged.
All 224 library/building/gunner-scan/map-load/export/save tests passed in
`target/audit-eternal-fire-tests.log`. This resolves the failure found by the
preceding status full-suite run, but does not establish a passing full suite.
The later concealed-building scan restart assertion now also expects its recovered
120-second interval. All 69 scan and three building-recovery tests pass in
`target/audit-woods-missile-complete-tests.log`; this resolves the next full-suite
failure without changing scan behavior or weakening the remaining state comparison.
Unknown-terrain substitution and diagnostics now use one shared decoder. Source
coordinates and original symbols reach the `MapErrors` channel for native/Lua
map creation, reload and load; pure asset inspection stays read-only. Elevations
and permanent fire flags are preserved. Existing channel delivery supplies
history, listeners and persistence; no second logging mechanism was introduced.
Map initialization and diagnostic publication now share an action checkpoint,
including when Lua catches a failure after one warning was already staged.
Channel absence is accepted, while malformed elevations and rows remain errors.
The 209 library tests and 16 map/diagnostic/foundation integration tests passed
in `target/audit-map-diagnostics-tests.log` and
`target/audit-map-diagnostics-final-tests.log`. Tests cover all three native/Lua
activation routes, successful restart, late callback rollback and second-message
overflow rollback. Independent map creation's random fire seed is the sole
explicitly normalized field in the native/Lua comparison.
Reference short-row rejection was verified; its newline and malformed-input
edge cases remain uncharacterized. Map validation reply strings remain open.

## Character and experience

Successful Computer shutdown overrides now award one point for an IC unit's
player pilot through the existing skill-award service. Computer's normal
threshold and strictly-more-than-30-second cooldown remain authoritative; this
site does not require a connected pilot, matching the reference helper. Failed
rolls, low-heat automatic avoidance and non-IC units receive no award. The heat
report carries the structured award and an accepted-award MechXP message, using
the existing thermal action's publication and rollback. No additional dice are
drawn for XP, and no separate counter or timer was added.

New biped/quad integration tests verify success/heat/IC/connection gates, exact
channel text, cooldown suppression after restart, and full heat/dice/XP/channel
rollback on publication failure. All three pass in
`target/audit-computer-xp-final-tests.log`; the initial test run's low-heat
expectation was corrected because a quiet check produces no report.
The 211 library tests and eight existing overheat/character-thermal regression
tests also pass, for 222 selected tests total. Results are in
`target/audit-computer-xp-tests.log` (library),
`target/audit-computer-xp-final-tests.log` and
`target/audit-computer-thermal-regressions.log`. Successful-award tests explicitly
compare the saved dice stream with exactly one Computer roll. All test targets
pass Clippy with warnings denied in `target/audit-computer-xp-clippy.log`.
Formatting and whitespace checks pass; the reference tree is unchanged.

Not yet addressed in this audit pass: chargen, advantage typing/effects,
non-IC pilot damage and initialization, personal combat/loadouts,
ejection XP reduction, consciousness retry messages, radio/computer failure and
recovery, character counters and character administration/query commands.

## UI and output

The preceding status-layout implementation is present; it is not evidence that
all audit UI requirements are satisfied. Still to verify/address: tactical/LRS
rules and glyphs, remaining status blocks/flags, additional armor
rendering modes and custom templates, weapon/charge/technology lines, weaponspecs
and shared menu rendering, roll statistics, base entry/radio messages and display
bounds. Unsupported-class display requirements remain explicit in the audit.

Implemented status-selector tolerance: unknown characters no longer cause an
error, recognized selectors still apply, and an all-unknown selection shows the
identification/condition header. Standard diagram snapshots remain unchanged.
Final focused verification passed nine status/gunner/water tests after the selector
change, including all eight diagram snapshots. Clippy with warnings denied and
whitespace checks passed. The reference tree remains unchanged.

Additional status work restored chassis-specific immobilization banners and
landed rotor-loss text, merged concurrent vehicle/inferno ON FIRE banners, and
converted limb recovery to rounded two-second display ticks. The 379 status,
gunner-report, movement and vehicle-fire regression tests passed, including all
eight unchanged diagram snapshots. Formatting and Clippy passed; this did not
complete full-suite verification or the remaining UI requirements above.

Physical-weapon readiness entries (axe, sword, claw, mace, saw) now follow
reference order, arm labels and limb/actuator checks, reusing existing actuator
queries. Coordinate target labels share reference spacing across chassis.
The 222 library/status/gunner-report tests and Clippy passed. Standard diagram
snapshots were unchanged; remaining UI requirements above are still open.


### Burst shells and occupied woods

The reference `combat/mech_hit_resolution.c` determines non-missile damage
once before choosing hit count. Ultra, rapid and rotary attacks then apply that
per-shell damage to each successful hit. Woods subtract two/four, with a minimum
of one, before the separate glancing damage reduction. Glancing also adjusts
the cluster roll. The terrain resolver and absorption notices run once for the
attack, with the original per-shell damage rather than burst total damage.

The Rust occupied-woods service now handles homogeneous direct-shell packets,
including bursts, instead of excluding all multi-round modes. Both target
adapters use that service. Burst cluster draws retain their glancing adjustment;
packet reduction and terrain effects remain shared. Lua report annotations now
make the per-shell versus missile-total distinction explicit.

The new acceptance matrix uses Mech/vehicle attackers and recipients, UltraAC/2,
UltraAC/5, RotaryAC/5 and rapid AC/5, both forest strengths, and ordinary/glancing
hits. It verifies shell count, per-shell damage and floor, single absorption
feedback, native/Lua state agreement, callback rollback and saved-state reload.
The first nine-test woods run passed before expanding the weapon matrix.

Pellet and thermal-only paths remain open. This change does not establish
burst-glancing parity with `moddamagewithwoods` disabled: that separate path
still needs review against the reference's per-shell halving. Full-suite
acceptance and other combat audit requirements remain open.


Expanded acceptance passed all 232 selected tests: 215 library, five Mech salvo,
three vehicle salvo and nine woods integration tests, recorded in
`target/audit-woods-burst-acceptance.log`. The first attempted new test read
Mech glancing from the vehicle report location; the assertion now reads the
existing chassis report wrapper correctly. No production behavior changed to
accommodate that test failure.

All targets pass Clippy with warnings denied in `target/audit-woods-burst-clippy.log`.
Formatting and whitespace checks pass; the reference tree remains unchanged.


### Burst glancing independent of woods configuration

Read-only tracing of `combat/mech_hit_resolution.c` confirms that non-missile
base damage is halved for a glancing unit hit before the burst hit-count lookup.
The later lookup separately lowers its roll for glancing. Both apply even when
woods damage is disabled, and low cluster rolls still produce one reduced shell.

A shared packet finishing operation now supplies that missing damage halving
for both Mech and vehicle targets when the occupied-woods service is not handling
it. Woods-enabled attacks continue to reduce damage after absorption, exactly
once. The packet operation consumes no dice and does not alter hit count.
Hex attacks retain their separate non-glancing path.

The native/Lua burst matrix now additionally covers grassland with woods damage
enabled and heavy forest with the option disabled. Existing light/heavy forest
cases guard against double halving. The same matrix checks saved-state reloads,
rollback, each supported burst mode, damage floors and target anatomy reuse.
This supersedes the woods-disabled glancing follow-up recorded above; pellet
and thermal-only woods handling and the broader audit remain open.

All 269 selected tests pass in `target/audit-burst-glancing-acceptance.log`:
215 library, five Mech salvo, 37 vehicle firing, three vehicle salvo and nine
woods tests. The new woods-disabled UltraAC/2 case initially missed because its
one-hex fixture exceeded a target number of twelve; the matrix now uses a firing
lane beyond minimum range. Successful-shot, glancing and damage assertions remain
strict, and the corrected cases pass through the normal native and Lua commands.

All targets pass Clippy with warnings denied in `target/audit-burst-glancing-clippy.log`.
Formatting and whitespace checks pass. The reference tree remains unchanged;
these selected checks do not establish full-suite or full-audit completion.


### Thermal-only hits and woods

The reference calls non-missile damage determination before returning from its
heat-mode flamer and coolant branches (`combat/mech_hit_resolution.c`). Thus
occupied woods receive the ordinary terrain check and absorption messages, while
thermal transfer uses catalogue damage rather than the reduced damage value.
Glancing likewise does not reduce heat or cooling strength.

The shared direct-effects service now uses the existing weapon packet profile
and occupied-woods resolver for successful thermal hits. The calculation can
consume terrain dice but does not roll a damage location or apply armor damage.
Both Mech and vehicle shot reports expose `thermal_woods`, whose feedback precedes
thermal notices. Native and Lua entry points share this path; both annotation
copies describe the optional report field. Range and component-damage inputs
come from the enclosing shot rather than a separate set of thermal formulas.

Acceptance covers Mech and vehicle carriers and recipients, Flamer, vehicle heavy
flamer and coolant, woods enabled/disabled and ordinary/glancing hits. It checks
unchanged catalogue transfer strength, intact material, woods report values and
single notification, callback rollback, native/Lua agreement and persisted state.
Miss-time non-missile terrain checks, pellet sequencing and broader terrain/output
parity remain separate work; this does not establish full combat parity.

Verification passed 262 selected tests: 37 vehicle firing and ten woods tests in
`target/audit-thermal-woods-tests.log`, plus 215 library tests in
`target/audit-thermal-woods-library.log`. The new native/Lua thermal matrix passes
for both unit families, both woods settings and glancing/non-glancing hits.

All targets pass Clippy with warnings denied (`target/audit-thermal-woods-clippy.log`).
Formatting and whitespace checks pass, and the reference tree remains unchanged.
Full-suite and full-audit completion have not been established.


### LBX pellet woods sequencing

The reference's non-missile damage determination performs a terrain check using
nominal LBX shell damage before choosing pellet count. `mech_missile_apply_hits`
then uses one damage per pellet and subtracts cover using the terrain left by
that first check. Thus heavy woods can thin before pellet absorption, and cleared
light woods no longer absorb pellets. The nominal reduced damage does not become
pellet payload.

Both target adapters now call a shared preliminary LBX woods operation before
cluster drawing. Their `initial_woods` report precedes the usual `woods` result.
The common projectile absorber now handles one-point pellets as well as missile
payloads and emits pellet-specific quantity wording. Ordinary shell and thermal
paths are unchanged. Both Lua annotation copies describe the extra result.

The new matrix covers Mech/vehicle attackers and recipients, LBX/2 and LBX/20,
light/heavy woods, both configuration values and multiple successful attack dice
streams. It checks surviving pellet counts and one-point groups against the
terrain produced by the preliminary check, full absorption, native/Lua state,
callback rollback and saved reloads. Assertions require both thinning and complete
clearing to have actually occurred. Miss-time terrain checks and broader combat
and output parity remain open.

The dedicated pellet matrix passed (`target/audit-pellet-woods-matrix.log`),
including observed thinning, full clearing and complete absorption. The shared
projectile report now places terrain feedback before absorption feedback, matching
`mech_missile_apply_hits`; direct-shell feedback retains its different reference
order. The broader regression run includes existing missile tests for this change.

The broad run passed 270 tests (215 library, five Mech salvo, 37 vehicle firing,
three vehicle salvo and ten woods tests) before reporting one stale Thunderbolt
feedback-order expectation. Actual feedback begins with `You clear 0,10.` before
the absorption pair, matching the reference sequence. The test now asserts that
concrete order; its targeted rerun is recorded separately. All new pellet matrix
cases passed in the broad run (`target/audit-pellet-woods-acceptance.log`).

The corrected Thunderbolt test passes in `target/audit-pellet-feedback-tests.log`,
for 271 distinct passing selected tests across the broad run and targeted rerun.
Clippy passes for all targets with warnings denied (`target/audit-pellet-woods-clippy.log`).
Formatting and whitespace checks pass; the reference tree remains unchanged.
Full-suite and full-audit completion remain unproven.


### Incidental terrain after non-missile misses

Tracing the callers resolves the prior miss-time uncertainty: the reference
`mech_fire_resolution.c` only enters ordinary non-missile hit resolution on a
hit. On a launched, in-range miss it instead calculates temporary nominal damage
(no woods absorption) and calls the terrain resolver with incidental intent.
That call does not depend on the woods-damage option. Swarming battle armor's
alternate-hit branch remains outside the supported unit scope.

The shared direct-effects service now performs that incidental check for launched
non-missile misses. Missile and pod paths keep their existing handling. Nominal
terrain damage reuses weapon profiles, range modifiers, component penalties and
gatling shot damage; bursts and LBX do not draw a hit count. Thermal woods hits
reuse the same nominal-damage helper. Miss reports expose `missed_terrain` and
publish its existing accidental-terrain notices without absorption feedback.

The acceptance matrix uses both source/target families, PPC, LBX cluster and
heat-mode Flamer, both woods settings and multiple forced misses. It compares
terrain outcomes and shooter dice with one attack draw followed by the existing
incidental resolver, requires actual clearing and ignition cases, verifies the
entire target unit remains unchanged, and checks native/Lua agreement, callback
rollback and saved reloads. Broader underwater, hex-shot, missile-miss and output
parity remain separate audit requirements.

The miss matrix passes in `target/audit-missed-terrain-matrix.log`. Its first
seed set covered no ignition events despite passing each state/dice comparison;
the fixture now selects deterministic quiet, clearing and ignition streams and
requires all three. No production behavior was changed to force those outcomes.
All 37 vehicle firing regressions also pass in
`target/audit-missed-terrain-regressions.log`.

Acceptance passes 263 distinct selected tests: 215 library and eleven woods tests
in `target/audit-missed-terrain-acceptance.log`, plus the 37 vehicle-firing tests
above. The already-verified, slower pellet matrix was excluded from this run;
its hit-only path was not changed. This is focused acceptance, not a complete
full-suite run.

All targets pass Clippy with warnings denied (`target/audit-missed-terrain-clippy.log`).
Formatting and whitespace checks pass; the reference tree remains unchanged.
The full audit remains active.


### Weaponspecs menu layout

The reference `wspec_fun` formats rows with `WSDUMP_MASK_*` and inserts them in a
one-column CoolMenu. `menu.c` uses 78 visible columns, truncates content at 77,
centers the title in blue bold, colors rules blue and the column header green,
and places a rule between the header and weapon rows. There are no pipe
separators or seconds suffixes in this display.

Rust now has a shared read-only information-menu renderer using existing styled
text width/truncation, including wide-character handling. `weaponspecs` uses its
fixed-width reference rows and model/reference title, escapes literal names and
publishes a Styled command report. Catalogue values, configured recycle times,
installation order and the two functional MML ammunition profiles remain shared
with structured weapon specifications. No simulation calculations were copied
into the renderer.

Focused acceptance checks rule/header placement, all 78-column row widths,
standard and extended-range layouts, a literal MediumLaser row, all supported
unit families and read-only runtime state. Existing weapon-report tests cover
native access, Lua structured data, damaged mounts and persistence. Interactive
and multi-column CoolMenu controls, other command migrations and `@stat` are not
implemented by this single-column information renderer.

Verification passes 221 distinct selected tests: 216 library tests in
`target/audit-weaponspecs-acceptance.log` and five weapon-report integration tests
in `target/audit-weaponspecs-final-tests.log`. The first new color assertion
expected the named-color markup spelling; truncation correctly canonicalizes
that spelling, so acceptance now checks rendered green/blue colors and boldness.
Exact row spacing and header text assertions remain unchanged and pass.

All targets pass Clippy with warnings denied (`target/audit-weaponspecs-clippy.log`).
Formatting and whitespace checks pass; the reference tree remains unchanged.
Full-suite and broader UI/audit acceptance remain open.


### Tactical audit corrections and invalid flags

Read-only tracing corrected the audit's LZ inversion claim. `aero_move.c` defines
`NO_ERROR` as zero and returns it for a suitable landing hex. The tactical overlay
therefore selects its misleadingly named `unsafe_marker` for that result; it is
plain `O` or the GOODLZ color marker, rendered green. Nonzero failures select `X`
with BADLZ red. Rust already matches this behavior, so reversing its markers
would introduce a regression. The existing saved team-exclusion/terrain scenario
now also verifies that monochrome output preserves the same glyphs and layout.

The reference `dohexlos` guard is enabled by darkness or the MW-only configuration
switch; the latter remains part of unimplemented MechWarrior support. For current
constructed families Rust's dark-map cliff/LZ rejection matches that guard. The
shared viewport already clamps tactical width and height to twice hardware range;
its boundary test explicitly checks a range of eight produces 16 by 16 cells.
The audit now distinguishes these verified behaviors from the remaining work.

Unknown single-letter tactical flags now return the exact reference text,
`Invalid tactical map flag.`, through both native and Lua paths. A new case tests
upper/lowercase unknown flags, unchanged full BattleTech state and no pending
output after rejection. This correction does not close all tactical admission,
MW/DropShip support, LRS or broader UI requirements.

The broad run passed 285 selected tests (216 library and 69 scan tests), including
the new invalid-flag case. The added monochrome assertion initially compared
escaped source text to rendered text; it now compares rendered text on both sides,
preserving literal bracket escaping. The targeted overlay rerun is recorded in
`target/audit-tactical-overlay-tests.log`; broad results are in
`target/audit-tactical-corrections-tests.log`.

The corrected overlay test passes, for 286 distinct passing selected tests across
the broad run and targeted rerun. All targets pass Clippy with warnings denied
(`target/audit-tactical-corrections-clippy.log`). Formatting and whitespace checks
pass. The reference tree is unchanged; the full audit remains active.


### Base-entry refusal wording

The reference `ui/mech_base_entry.c` distinguishes jump, prone/standing,
uncontrolled flight, VTOL fuel and landing requirements before checking speed
and routes. Rust's generic map-transfer guard previously hid these distinctions
behind one airborne-transfer error, and its prone and excess-argument replies
used different text.

The shared map-transfer check now returns a typed reason internally; generic
transfers retain their admission and error text. Building entry uses that reason
for the reference jump, uncontrolled-flight and landed-VTOL messages, alongside
the reference prone/standing and excess-argument replies. Movement predicates
remain in one service, and native/Lua/countdown rechecks retain the existing
shared entry path. The change creates no additional timers or state flags.

New native/Lua cases verify prone, jumping and airborne-VTOL refusals with full
BattleTech state equality and no staged output. Existing entry cases continue to
cover callbacks, delayed movement, locks, parser behavior and rollback. The first
run passed thirteen tests and found the old `Usage:` expectation for excess
arguments; it now checks `Invalid arguments to command!` exactly. Broader
refusal precedence, route/lock diagnostic text and unsupported families remain
open requirements.

All 216 library and fifteen host-entry tests passed in
`target/audit-base-refusals-acceptance.log`. The lower-level transfer regression
then found its blanket generic-error expectation for all three APIs; it now
requires the base-entry-specific mid-flight reply only for entry, while preserving
the exact generic reply for direct transfers and exits. Its saved falling-state
and no-movement assertions remain intact. The host matrix additionally covers
an upright unit still completing its stand timer.

Final entry/transfer regression passed seventeen integration tests in
`target/audit-base-transfer-final-tests.log`; with the 216 library tests, 233
distinct selected tests pass. Clippy passes all targets with warnings denied
(`target/audit-base-refusals-clippy.log`). Formatting and whitespace checks pass.
The reference tree is unchanged, and full-audit completion remains open.


### Radio frequency syntax and audit correction

The reference `mech_notify_radio_config.c::radio_frequency_is_valid` accepts only
ASCII decimal digits. Negative text therefore fails with `Invalid frequency!`
before the later `freq < 0` branch, so the audit's requested `Are you trying to
kid me?` message is unreachable through this command. Rust's negative-input reply
was already correct.

Rust's native parser did accept a leading plus sign and unsigned values above the
reference signed-integer range. It now validates digit-only syntax and the signed
parser boundary before calling the existing shared frequency setter. Thus signed
parser overflow gets `Invalid frequency!`; values up to 2147483647 but above the
radio maximum still get `Invalid frequency - range is from 0 to 999999.`. Zero,
leading zeroes and 999999 remain valid. Typed Lua/Rust setters retain their shared
numeric-domain validation and transactional audit publication.

The new native boundary matrix checks all those cases, Unicode digits, decimal
and exponent notation, large overflow, exact replies, unchanged BattleTech state
and no staged output on rejection. The audit now records the actual executable
behavior instead of requesting an unreachable negative-value branch.

The initial run passed 216 library and 25 radio tests. The new boundary case
found that overflow retained Rust's parser detail after `Invalid frequency!`;
native parsing now maps that failure to the exact reference reply. The radio
suite rerun is recorded in `target/audit-radio-parser-final-tests.log`; initial
results are in `target/audit-radio-parser-tests.log`.

All 26 radio tests now pass; together with the 216 library tests this gives 242
distinct passing selected tests. Clippy passes all targets with warnings denied
(`target/audit-radio-parser-clippy.log`). Formatting and whitespace checks pass,
and the reference tree is unchanged. Broader audit work remains active.

## LRS coordinate headings and height limits

The reference `ui/mech_lrs_map.c::show_lrs_map` emits exactly three heading
rows, selecting the first three characters of each minimum-width-three decimal
coordinate. Rust previously expanded every label to four rows when a viewport
crossed 999. The shared LRS renderer now preserves three rows and the reference
prefix truncation, without changing hex placement or sensor filtering.

Boundary tests cover 9/10, 99/100, 999/1000 and 9999/10000, with literal expected
headings and staggered hex rows. Additional viewport tests cover odd/even
preferences, the twice-range limit, both map edges and short maps. The audit's
odd-height gap was a false positive: both implementations make the requested
height odd and then clip displayed rows to map bounds, so a short even-height
map displays all its rows without padding.

All 218 library tests and 70 scan tests pass in
`target/audit-lrs-label-unit-tests.log` and
`target/audit-lrs-label-scan-tests.log`. This is focused verification, not a
completed full-suite run. LRS stacking remains open: the reference's positional
exchange ordering can change which co-located unit is displayed, while Rust
currently chooses the first membership slot and explicitly prioritizes self.
Additional glyphs depend on unit families that still require implementation.

All-target Clippy with warnings denied passes in
`target/audit-lrs-label-clippy.log`; formatting and whitespace checks also pass.
The reference tree remains unchanged. The broader audit goal remains active.

## LRS stacked-unit selection

The shared LRS renderer now selects stacked markers after positional exchange
ordering, starting with persisted battlefield membership order. This reproduces
the visible reference behavior where moving an earlier coordinate through the
list can change the winner among equal-position contacts. A stable coordinate
sort would not reproduce that behavior. The observer participates in the same
selection instead of overriding its hex with `*` after other contacts are drawn.

Non-observer contacts are filtered by viewport bounds before selection. The
reference includes the first row below the displayed rectangle in its candidate
list; the Rust bounds preserve that detail. Contacts still require acquired
visibility, and affiliation colors travel with the selected marker. One shared
selection path handles all constructed unit types and both LRS unit modes.

Tests cover direct ties, intervening earlier rows/columns, later positions,
observer precedence, and native/Lua agreement in both modes. The integration
matrix also checks invisible contacts and an earlier coordinate outside the
viewport, unchanged complete state, no staged output, and save/reload agreement.

All 219 library, 71 scan and six vehicle scan tests pass (296 distinct selected
tests). Logs are `target/audit-lrs-stacking-unit-tests.log` and
`target/audit-lrs-stacking-final-tests.log`. The first broader run exposed an
existing vehicle-scan restart expectation: loading initializes the damaged
building's absent repair timer to 120. That was the only whole-state difference;
the test now explicitly accounts for it while comparing all remaining state.
The production building lifecycle is unchanged.

All-target Clippy with warnings denied, formatting and whitespace checks pass
(`target/audit-lrs-stacking-clippy.log`). This is focused verification, not a
completed full-suite run. The reference tree is unchanged; remaining unit-family
glyphs and the broader combat, movement, sensor, map, character and UI audit
requirements remain active.

## SETCOND parser and refusal parity

The native environment command now emits the reference's exact required-field,
gravity, temperature, vacuum and underground replies. Malformed numbers, signed
integer overflow and out-of-domain values use the same field-specific message,
without appending Rust parser diagnostics. Validation preserves field order.
The temperature reply intentionally retains the reference's missing closing
parenthesis.

Read-only inspection of `map/map_conditions.c` and
`scripting/functions.c::mech_parseattributes` also exposed two behavioral gaps:
signed zero is valid, and tokenization stops at four space/tab-delimited fields.
The reference's excess-argument branch is therefore unreachable. Native Rust
now accepts signed zero and ignores trailing fields. Typed Lua environment
updates still use the same bounded structure and shared transactional setter;
no environmental simulation rules or per-unit caches were added.

Tests compare exact native rejection replies and unchanged whole BattleTech
state, with no staged output. The successful native/Lua matrix covers signed
zero and ignored trailing arguments across all seven supported unit templates,
alongside existing flag retention, callback rollback and restart checks.

All 219 library and five environment tests pass (224 distinct selected tests),
with results in `target/audit-setcond-unit-tests.log` and
`target/audit-setcond-final-tests.log`. All-target Clippy with warnings denied,
formatting and whitespace checks pass (`target/audit-setcond-clippy.log`). The
reference tree is unchanged. The broader audit remains active; this focused
verification does not establish a green full suite.

## LOADMAP structural failure replies

The audit's `map_checkmapfile` reply item belongs to `LOADMAP` preflight, not a
separate CHECKMAP command. Structural decoding now carries typed dimension and
row failures; bounded asset reads carry an unavailable-file category. Existing
inspection paths retain detailed diagnostic chains. Activation through LOADMAP
maps these categories to the exact reference replies without matching error
strings or introducing another parser.

Native failures include `Loading <name>` followed by the reference reply for
missing assets, invalid dimensions or incomplete rows. Missing arguments use
`Invalid number of arguments!`; extra arguments are ignored because the
reference tokenizer reads only the first token. Replies remain literal text.
Failed loads leave world state and staged effects unchanged. Successful loads
retain the shared terrain, membership, shutdown and rollback services.

The tests check direct and Lua failure messages, exact native output, trailing
arguments, missing input, whole-state preservation and empty staged output.
The existing successful native/Lua matrix now includes trailing native arguments
across seven supported unit templates and both operator roles, preserving its
callback rollback and save/reload checks.

Malformed-row acceptance (including newline counting), non-UTF-8 input and
failed-load MapErrors channel publication remain outstanding. This change
establishes the standard failure replies, not complete reference parser or
preflight parity. The broader audit remains active.

All 219 library and nine selected map tests pass (228 distinct tests). Logs are
`target/audit-map-preflight-unit-tests.log`, `target/audit-map-preflight-tests.log`
and the final load-test rerun `target/audit-map-preflight-final-tests.log`.
All-target Clippy with warnings denied, formatting and whitespace checks pass
(`target/audit-map-preflight-clippy.log`). The reference tree is unchanged. A
full-suite pass has not been established by these focused checks.

## Bounded map records and ignored row suffixes

Map decoding now reads the same bounded records as the reference: up to 63
bytes for the dimension line and 2001 bytes for each terrain row, retaining
line endings in the consumed record. Only the declared tile pairs are decoded;
trailing bytes are discarded. A full record without a newline continues into
the following row. NUL terminates the visible record without leaving buffered
suffix bytes for the next read. This replaces exact-line-length rejection with
one shared decoder used by inspection and activation.

The malformed short-row audit is now characterized more precisely. A newline
can satisfy the reference preflight length check and then occupy an elevation
slot. Subtracting ASCII zero produces a negative elevation, which aborts in
`map_coding_id_slot`; oversized elevations likewise fail checked registry bounds.
Rust retains a normal error for invalid elevations instead of reproducing a
process abort. This is an explicit error-handling difference, not claimed
successful loading of a reference-supported tile.

Tests cover ignored suffixes, final rows without newlines, embedded NUL, bounded
record continuation, maximum-width LF/CRLF input and invalid short rows. Existing
native/Lua creation, reload and LOADMAP matrices now use suffixed terrain rows,
checking exact tile counts, terrain diagnostics, conditions, rollback and restart
across supported unit types. Non-UTF-8 assets, metadata boundaries and failed-load
channel publication remain follow-up work within the active broader audit.

All 220 library and six selected map tests pass (226 distinct tests), recorded
in `target/audit-map-record-final-unit-tests.log` and
`target/audit-map-record-final-tests.log`. All-target Clippy with warnings denied,
formatting and whitespace checks pass (`target/audit-map-record-clippy.log`).
The reference tree remains unchanged. These focused checks do not establish
a complete full-suite pass.

## Optional map metadata

The map decoder now reads only the first post-terrain record, bounded to 2002
bytes as in `map_load`. It applies conditions only when the record contains
three valid signed integers with the expected colon and two condition fields.
Malformed, partial and overflowed records leave default gravity/temperature
and inferred permanent-fire flags intact. Valid gravity and temperature clamp
to their storage ranges; trailing records are ignored. Blank records are not
skipped to search for later metadata, and embedded NUL ends the visible record.

Unit tests cover fallback, all integer-overflow positions, signed zero, clamping,
record boundaries, blank/extra records and NUL termination. Native/Lua creation,
reload and LOADMAP matrices cover fallback and clamping while preserving exact
terrain diagnostics, complete state agreement and restart behavior.

Read-only comparison also identified a remaining lifecycle difference: when
metadata is absent or invalid, the reference retains existing map flags and ORs
in authored fire. Fresh asset decoding has no existing map flag context, so
activation must still supply that context. This and remaining lexical/non-UTF-8
cases stay open in the audit; the optional-metadata change does not claim full
loader parity.

All 221 library and eight selected map tests pass (229 distinct tests), with
logs in `target/audit-map-metadata-unit-tests.log`,
`target/audit-map-metadata-tests.log` and
`target/audit-map-metadata-final-tests.log`. All-target Clippy with warnings
denied, formatting and whitespace checks pass
(`target/audit-map-metadata-clippy.log`). The reference tree is unchanged and
the broader audit remains active. No complete full-suite pass is claimed.

## Existing flags during file reload

File activation now supplies the current map's flags to the shared decoder.
Absent or malformed optional metadata retains those flags and adds the authored
permanent-fire bit. Valid metadata replaces the complete flags value, including
an explicit zero. Standalone file inspection and new maps still begin with zero.
This adds no saved metadata provenance or separate parsing/merging logic.

The decoder tests compare fresh and inherited contexts across ordinary terrain,
authored fire, absent/invalid metadata and explicit replacements, including
negative flag masks. Native/Lua create, reload and LOADMAP tests now initialize
existing maps with multiple unrelated flags and verify preservation or replacement
as appropriate, full state agreement, diagnostics and restart.

All 222 library and eight selected map tests pass (230 distinct tests), recorded
in `target/audit-map-inherited-flags-unit-tests.log` and
`target/audit-map-inherited-flags-tests.log`. Remaining lexical/non-UTF-8 cases,
failed-load channel publication and the broader audit remain active.

All-target Clippy with warnings denied, formatting and whitespace checks pass
(`target/audit-map-inherited-flags-clippy.log`). The reference tree is unchanged.
These focused checks do not establish a complete full-suite pass.

## Byte-oriented map assets

Map files no longer require whole-file UTF-8 validity. A shared bounded and
directory-confined byte reader serves map decoding; text template readers retain
UTF-8 validation over that same reader. The map decoder consumes bytes directly,
so uninterpreted row suffixes and ignored later records can contain arbitrary
bytes without blocking activation.

Unknown terrain bytes use the existing grassland substitution and diagnostic
path. Invalid elevation bytes still reject atomically; invalid dimension bytes
reject, while invalid optional metadata falls back. Diagnostic terrain bytes
are represented as Unicode characters, preserving valid Rust output strings.
No filesystem confinement or size limits were relaxed.

Unit tests cover non-UTF-8 terrain, suffixes, metadata, dimensions and elevations.
The native/Lua create/reload/LOADMAP matrix now uses non-UTF-8 row suffixes and
continues checking exact diagnostics, whole-state agreement, inherited flags,
metadata replacement and restart. All 223 library and eight selected map tests
pass (231 distinct tests), recorded in `target/audit-map-bytes-unit-tests.log`
and `target/audit-map-bytes-tests.log`. Remaining lexical cases, failed-load
channel publication and the broader audit remain active.

All-target Clippy with warnings denied, formatting and whitespace checks pass
(`target/audit-map-bytes-clippy.log`). The reference tree is unchanged. These
focused checks do not establish a complete full-suite pass.

## Map numeric token boundaries

Map dimensions and optional metadata now share explicit file-field tokenization
and signed-integer parsing. Space, tab, carriage return and newline delimit fields;
the numeric parser additionally accepts form-feed and vertical-tab around an
integer. Those two characters do not separate adjacent numbers. Unicode spaces
are not file delimiters or accepted numeric padding. Dimensions pass through the
signed parser before the existing 1–1000 bounds, while metadata retains its
whole-record fallback and clamping rules.

Tests cover signed dimensions, accepted ASCII padding, missing separators,
Unicode whitespace, integer overflow and zero dimensions. The native/Lua
creation/reload/LOADMAP matrix now exercises padded metadata and Unicode-field
fallback with preserved flags, diagnostics and saved state. Failed-load channel
publication and the broader audit remain active.

All 224 library and six selected map tests pass (230 distinct tests), with logs
in `target/audit-map-numeric-unit-tests.log`, `target/audit-map-numeric-tests.log`
and `target/audit-map-numeric-final-tests.log`. All-target Clippy with warnings
denied, formatting and whitespace checks pass
(`target/audit-map-numeric-clippy.log`). The reference tree remains unchanged.
These focused checks do not establish a complete full-suite pass.

## Failed-load MapErrors publication

LOADMAP dimension and row failures now publish the exact reference preflight
diagnostics through the shared MapErrors channel service. Missing-file failures
remain silent on that channel. Map activation first restores its state and
staged effects, then publishes the diagnostic as a separate atomic step; channel
capacity or output validation failure restores that publication attempt.

Native and Lua use the same host action. A caught Lua rejection retains its
diagnostic within the successful enclosing callback, while an unhandled error
or later callback abort restores it. Lua returns the operation error outside
the inner diagnostic transaction, preserving both behaviors without suppressing
the load error or duplicating publication logic.

The new matrix checks exact diagnostic text for bad dimensions and incomplete
rows, missing-file silence, unchanged complete BattleTech state, native/caught
Lua publication, saved channel history, callback rollback, and channel-counter
overflow with no staged output. Existing no-channel and successful-load tests
remain passing.

All 224 library and seven selected map tests pass (231 distinct tests), with
results in `target/audit-map-failure-channel-unit-tests.log` and
`target/audit-map-failure-channel-tests.log`. The broader combat, movement,
sensor, map, character and UI audit remains active.

All-target Clippy with warnings denied, formatting and whitespace checks pass
(`target/audit-map-failure-channel-clippy.log`). The reference tree remains
unchanged. These focused checks do not establish a complete full-suite pass.

## Full-suite regression pass: metadata and missed-shot rolls

The first new full-suite run stopped after 250 passing tests at an older
`tests/btech.rs` assertion that malformed optional metadata must reject the map.
That assertion now verifies default flags/gravity/temperature and intact terrain,
matching the reference's whole-record metadata fallback.

The rerun stopped after 1,220 passing tests with five failures in `btech_motion`.
They expected missed direct shots to consume only the attack roll before recoil
or comparison of the shooter stream. The implemented reference incidental terrain
checks also consume dice on non-wooded terrain, before heavy-Gauss recoil. Tests
now include the independent expected check sequence, preserving complete-state
comparisons, exact recoil outcomes, damage, expenditure, XP and rollback checks.
Recoil seed selection accounts for those checks. One test's 256 repeated-byte
seeds contained no required miss/recoil pair, so its deterministic search now
covers a two-byte seed domain without relaxing either required roll.

All 357 movement tests pass across `target/audit-miss-roll-motion-tests.log`
(356 passing tests) and `target/audit-miss-roll-recoil-tests.log` (the corrected
remaining case). All-target Clippy with warnings denied, formatting and whitespace
checks pass (`target/audit-miss-roll-clippy.log`). No combat production code was
changed to satisfy these expectations, and the reference tree is unchanged.

A third full-suite run is in progress in `target/audit-current-full-suite-third.log`.
Its terminal result must be checked before claiming full-suite success. The
broader audit remains active, including underwater firing: reference review
confirmed distinct catalogue water ranges, mount-level submersion, and separate
target waterline/LOS constraints shared by sighting and firing.

## Full-suite recovery heartbeat expectation

The third full run passed all 357 movement tests and stopped after 1,343 passing
tests at `recovery_ticks_without_a_unit_and_failed_save_replays_the_same_roll`.
The only complete-state difference was the committed shared turn phase: one
instead of zero. Its expected successful heartbeat now includes phase one;
the injected failed-save assertion still compares against the unchanged complete
pre-tick state, including phase zero and the original recovery dice.

All three recovery tests pass (`target/audit-recovery-phase-tests.log`).
All-target Clippy with warnings denied, formatting and whitespace checks pass
(`target/audit-recovery-phase-clippy.log`). Production recovery code and the
reference tree remain unchanged.

A full `cargo test --no-fail-fast` run is now active, logging to
`target/audit-current-full-suite-all.log`. It will collect failures across all
test binaries. Its terminal result remains unverified; no full-suite success or
completion of the broader audit is claimed.

## Full-suite sensor, admission and help expectations

The complete no-fail-fast run terminated with 2,538 passing tests and four
failures (`target/audit-current-full-suite-all.log`). All four are corrected
expectations, with no production gameplay changes:

- Gatling sensor-order replay now includes the reference incidental-miss
  ignition/clearing rolls after launch, while sighting remains read-only.
- Automatic light rechecks preserve a vehicle's coordinate target; completing
  an explicit sensor change still clears the unit lock.
- The native command catalog recognizes public cockpit VIEW admission. Its
  map-room wizard restriction remains enforced by contextual command admission.
- Live HELP compares complete persisted game state while allowing only the
  independently advancing BattleTech phase to differ. Comparing SQLite bytes
  during that heartbeat incorrectly treats clock persistence as a help mutation.

All 11 scanner-chassis, vehicle-targeting and sensor-light tests pass in
`target/audit-sensor-replay-expectations.log`. The catalog and TCP help regression
checks each pass in `target/audit-view-catalog-permissions.log` and
`target/audit-help-heartbeat-expectation.log`. Formatting and diff checks pass.
The full suite has not been repeated after these corrections; full-suite success
and completion of the broader audit remain unproven.

## Underwater range catalogue and arithmetic

`src/btech/equipment/water.rs` adds one unit-independent water profile for each
of the 32 eligible catalogue weapons. All four numeric fields were checked
against the active reference catalogue; commented-out weapon records are not
included. Missing long bands are represented explicitly, preserving the
reference medium-range cap and its refusal to extend these weapons.

The shared water range query rounds before maximum and intermediate checks,
retains the zero-range PPC penalty, and preserves positive minimum-range
fallthrough to ordinary catalogue minimum arithmetic. Unsupported weapons use
`This weapon may not be fired underwater.`; invalid numeric distances are errors
and legal out-of-range shots return no bracket. No dice or unit state is touched.

All 228 library tests and five aim/weapon-damage integration tests pass in
`target/audit-water-range-acceptance.log`. This is a prerequisite, not live
underwater-combat acceptance. The current admission gates remain in place.
Next integration must pass mount submersion into both aim adapters and the
coordinate path, apply water C3 limits without extreme range, and carry water
bands into shared damage sizing before removing those gates. Underwater C3 uses
physical distance to limit reach but peer distance for its short/medium/long
band; minimum-range network admission still needs to follow the enclosing
reference control flow. Waterline LOS and section-specific submersion remain
separate geometry requirements.

All-target Clippy with warnings denied passes (`target/audit-water-range-clippy.log`).
Its status-layout warning was resolved by keeping Mech technology entries in one
shared chassis block; all 13 status tests still pass
(`target/audit-water-status-check.log`). Formatting and diff checks pass, and the
reference tree is unchanged.

## Underwater mount and aim integration

Unit and coordinate aim now apply shared mounting-section submersion before
network assistance and target stealth. `weapon_geometry` owns the submersion
query used by both aiming and bearing checks; there is no new persisted flag.
Shallow-water legs and quad front limbs use water ranges, torso mounts remain
above water while standing, and prone Mechs submerge all mounts. Vehicle and
VTOL queries retain their actual elevation; hover and landed flight height are
not inferred from map depth.

C3 receives the same submersion fact, uses water short/medium bands and refuses
physical ranges beyond the unextended water maximum. A live network test checks
both unit and coordinate aim, a close peer, loss of physical reach and restart.
Reference `mech_bth.c` handles raw physical minimum range before calling either
bracket routine, so the main aim query retains that precedence: a colocated PPC
has modifier four even though the low-level water bracket routine returns three.

The initial library/water/C3 acceptance passes 264 tests
(`target/audit-water-aim-tests.log`). The added live water-network boundary test
passes (`target/audit-water-aim-boundaries.log`), and all 19 water, ordinary aim,
sighting, vehicle aim and vehicle hex-fire regressions pass
(`target/audit-water-aim-regressions.log`), including the new PPC precedence test.
Live underwater firing remains gated until damage and sight/fire admission are
connected and verified together.

Damage integration can carry submersion through the existing private
`salvo::ShotDamage`, `vehicle_salvo::SalvoContext` and
`direct_effects::DirectEffectRequest` into `WeaponGroupRequest`; fixed external
salvo calls can retain their ordinary range environment. This avoids inferring
mount position from weapon identity when a unit has multiple identical weapons.
The missing-long-band sentinel also matters to range-modified damage: after the
one-hex bonus branch, the reference halves damage when that long band is absent.

All-target Clippy with warnings denied passes in
`target/audit-water-aim-clippy.log`. Formatting/diff checks pass, and the reference
tree remains unchanged. Full audit completion is not claimed.

## Live underwater combat

The blanket submerged-shooter/target rejection is replaced by the shared
mount-level eligibility gate. Mech and vehicle unit shots, sighting, and empty
hex shots all consult that gate; LOS still controls the actual waterline.
The exact unsupported-weapon message is
`This weapon may not be fired underwater.`. No launch expenditure or dice is
committed for that rejection.

Submersion travels through private shot, vehicle-salvo and direct-effect contexts
into the shared weapon packet builder. Public fixed-direction salvo inputs are
unchanged. Range-modified energy damage applies focusing damage first, then
water medium/long thresholds, then glancing. A missing water long band retains
the reference halving beyond one hex; the one-hex bonus still takes precedence.
Swarm continuation remains an ordinary missile context and never uses energy
range damage. Thermal/miss terrain-strength calculations and coordinate impacts
use the same packet sizing path.

All 248 focused library/sighting/underwater/vehicle-hex/water-accuracy tests pass
in `target/audit-underwater-combat-acceptance.log`. Actual underwater shots cover
biped, quad, tracked, wheeled and stationary shooters against Mech and vehicle
recipients, with host range damage enabled and disabled, native/Lua equality,
callback rollback and saved replay. Sight consumes its expected attack roll only;
its geometry and range changes do not mutate other unit state. Empty underwater
hexes and blocked air/water visibility are checked separately.

The broader movement and vehicle-combat run passes all 409 tests in
`target/audit-underwater-combat-regressions.log`: 357 movement, 37 vehicle fire,
three vehicle-shot admission and 12 woodland tests. Two existing rejection tests
now assert the specific rule: a submerged ineligible weapon, or a prone shooter
whose sightline crosses the waterline. All-target Clippy with warnings denied
passes (`target/audit-underwater-combat-clippy.log`), as do formatting and diff
checks. The reference tree is unchanged. Full-suite success and completion of
the broader audit remain unproven.

## Shared TAG across supported unit families

The sensor audit led to a functional gap beyond its listed TAG replies: the
selection, ownership lookup and clock only visited constructed Mechs. TAG now
uses one rule implementation across Mechs and vehicles, including illumination
of either target family and replacement of an illuminator from the other family.
Vehicle TAG state is serialized in the existing vehicle record; no second
ownership index or separate vehicle timer implementation was added. Validation
checks uniqueness across both stores. Shutdown, tactical membership changes,
the live-work predicate, Lua inspection and cockpit status include vehicle TAG.

Standalone TAG components and integrated C3 masters use existing component
availability and command-computer accounting. Selection distinguishes missing
hardware from destroyed hardware before parsing native arguments or resolving
targets. Native TAG accepts battlefield labels as well as dbrefs and uses the
reference invalid-target, argument-count and range replies. The fifteen-hex
limit compares continuous distance without rounding. Success names use the
observer's visible contact and escape literal markup.

`tests/btech_tag.rs` covers seven source chassis against Mech, ground-vehicle
and VTOL targets, native/Lua equality, callback rollback, restart, thirty-second
lock and recycle boundaries, shutdown, cross-type takeover, corrupt duplicate
ownership, C3-integrated TAG, equipment failure, argument precedence and exact
versus fractional range boundaries. A live vehicle illuminator also removes the
movement penalty for semi-guided Mech and vehicle launchers; equipment loss
removes that assistance immediately, before the next clock tick.

The cable-command investigation established a prerequisite rather than a new
cockpit command: reference ATTACHCABLES and DETACHCABLES use class flag 128,
GFLAG_MW. On-foot MechWarrior construction and actor admission are still needed.
The commands remain in scope; exposing them from arbitrary Mechs or vehicles
would bypass the reference class restriction. Reference sources were read only.

The full six-section audit remains active. This work does not establish complete
TAG lifecycle/admission parity or close unrelated sensor and UI report gaps.

Verification: 270 distinct focused tests passed. This includes 230 library tests,
13 status tests, six cross-chassis TAG tests, five existing Mech TAG/semiguided
tests, and 16 C3 hardware, map-assignment and vehicle power/state/storage tests.
Logs: `target/audit-tag-chassis-tests.log`, `target/audit-tag-final-tests.log`,
`target/audit-tag-mech-regressions.log` and
`target/audit-tag-renderer-regressions.log`. All-target Clippy with warnings as
errors passes in `target/audit-tag-clippy.log`; formatting and diff checks pass.
The full suite was not rerun for this change.

## Typed advantages and shared boolean semantics

The character audit now has a single 22-entry advantage catalog: thirteen
boolean advantages, eight ranked values and the Exceptional_Attribute bit mask.
The Rust catalog and `btech.character.advantages()` provide canonical names and
kinds. Lua results are detached. Existing character values retain their exact
raw names, values, XP and last-use metadata; this change adds no side effects to
raw value assignment and no additional persisted advantage state.

The reference `has_bool_advantage` compares the stored value to exactly one.
More than thirty Rust consumers instead used positive-value checks and exact
keys. They now share the catalog's case-insensitive boolean interpretation,
alongside the existing shared boolean reader. This changes actual toughness,
pain-resistance and melee-specialist behavior in injury, fall, shutdown, heat,
physical combat and movement paths; ranked values and attribute masks are not
boolean switches. Existing pilot selection and explicitly supplied rule policy
remain owned by their original action.

The audit's life-accounting claim was inaccurate. Reference EE_NUMBER is 11,
which indexes ShotsHit in the current catalog. Exceptional_Attribute is index
20 and Extra_Edge is index 21. The numeric setter's Lives adjustment therefore
cannot prove the claimed Exceptional_Attribute behavior. The discrepancy is
recorded in the audit; no guessed life-accounting behavior was introduced.

Focused tests cover the complete catalog and Lua detachment, raw values
0/1/2/255 and mixed-case names, pain-resistance injury thresholds across all
seven supported chassis, persisted replay, moving shutdown saving checks for
Mechs/vehicles, and melee-specialist DFA damage/aiming. Equivalent cases compare
complete game state while excluding only the deliberately varied raw input.
Character and recovery regression suites also pass. The full six-section audit
remains active, including remaining advantage effects, chargen and life policy.

The focused run passes 22 character, recovery and melee tests in
`target/audit-advantages-tests.log` and `target/audit-advantages-melee.log`.
The 230 library tests also passed in `target/audit-advantages-library.log`.
A read-only independent comparison verified all 22 catalog names, kinds and
ordering against the reference. Formatting and diff checks pass; the reference
working tree is unchanged. A full `cargo test --no-fail-fast` run is in progress
in `target/audit-advantages-full-suite.log`, followed by all-target Clippy in
`target/audit-advantages-clippy.log`. Their completion is not yet established.

## Forward-observer lifecycle characterization

While the full advantage regression run continues, the next sensor gap has been
traced through `mech_spot.c`, numbered weapon lookup, event scheduling and unit
shutdown/destruction. `docs/btech-spotter-datalink.md` records the admission,
spotter-owned radio range, delay formula, four-coordinate movement conjunction,
participant notices, multiple outstanding requests and ten-second maintenance
behavior. It corrects two implementation traps: merely having installed
artillery is insufficient when its numbered lookup is recycling, and replacing
a single pending slot would discard observable queued requests. Empty ammunition
alone does not reject this particular lookup. No delayed datalink implementation
or acceptance completion is claimed yet.

The existing full test run completed compilation and is executing test binaries.
It is still in progress; its final result and the queued Clippy result must be
inspected before claiming broad verification. No second full run was launched.

The ongoing full run exposed a nondeterministic setup in
`underwater_network_aim_keeps_the_physical_water_limit`: moving the units to the
water map erased the field fixture's known contacts, and a random optical refresh
was then used to reacquire the peer before C3 admission. The test is about water
range after acquisition, not detection probability. It now reinstates explicit
acquired contacts, consistent with its parent fixture, and separately asserts
current contact visibility and unblocked terrain LOS. Runtime sensor and C3
rules were not changed. All 26 C3 tests pass in
`target/audit-water-c3-acquisition-fixture.log`. The original full run continues
against its already-built test binaries, so its recorded failure remains in
that log; the correction is verified by the separate focused run.


## Forward-observer implementation and completed baseline results

The earlier full advantage baseline finished: 332 suites, 2,566 passed and one
failure, the subsequently corrected underwater C3 acquisition fixture. Its
full result is in `target/audit-advantages-full-suite.log`. The current 26-test
C3 rerun passes. The baseline's queued Clippy exposed a helper after a test
module; it has been moved. The changed spotter result also required adapting
one older single-notice test caller. Current all-target Clippy passes in
`target/audit-spotter-links-clippy.log`. No full green result is claimed.

The delayed forward-observer service is now implemented with one durable event
queue type shared by Mechs and vehicles. Tests exercise all supported chassis,
exact radio/delay boundaries, the four-coordinate cancellation conjunction,
multiple requests and insertion order, clear/shutdown/destruction, restart,
transactional native/Lua output and live server save failure. Artillery uses
completed links without an alternate firing path. The common correction reset
traversal now preserves correction on declaration/clearing, resets the shooter
on direct selection, and clears dependent shooters when an observer withdraws.
An older gunner test incorrectly expected clearing the parent link to discard
its correction; its expectation now follows the reference early-return branch.

`docs/btech-spotter-datalink.md` records current evidence and outstanding edge
acceptance. It supersedes the earlier characterization-only status. The broad
Combat, Movement, Sensors, Map/special objects, Character/experience and UI/output
goal is still active; this sensor increment does not narrow that scope.

## Standard scan armor and datalink maintenance edges

Ordinary scans now share the status armor renderer for all eight supported
silhouettes. The same section masks and typed cells render qualitative damage
symbols and the reference three-column Key legend. Protection colors and
integer thresholds have one implementation for owned and adversarial output;
owned cockpit snapshots remain unchanged. The obsolete per-section armor lists
and their duplicated Mech/vehicle formatting have been removed.

Verification: 94 scan/status integration tests pass in
`target/audit-scan-diagrams.log`, including all silhouettes through native/Lua,
restart, numeric-disclosure boundaries and read-only state. Four renderer/export
unit tests pass in `target/audit-scan-diagrams-unit.log`; all-target Clippy passes
in `target/audit-scan-diagrams-clippy.log`. Formatting and diff checks pass.
`docs/btech-scan-display.md` retains the outstanding information-block,
weapon-table, repair and unsupported-family work.

Datalink maintenance now observes cleared selection before resolving its retained
observer, avoiding a false lost-link warning if that observer is subsequently
deleted. First-critical loss and same-section physical recycling have explicit
eligibility tests. All 14 link tests pass in `target/audit-spotter-link-edges.log`.
The reference tree is unchanged. This is focused verification; the full six-section
audit remains active and no new full-suite result is claimed.


## Shared scan weapon table

`scan_weapons.rs` replaces duplicate Mech/vehicle scan weapon formatting with
one fixed-column renderer and small anatomy adapters. It preserves the reference
headers, prefix-free/truncated names, location columns, first-critical damage
marker and nonzero recycle marker. Destroyed sections are omitted and display
numbers compact independently of stable weapon state indexes. This does not
change firing admission or expose ammunition.

All 97 focused integration tests pass: 94 existing scan/status scenarios in
`target/audit-scan-weapons.log` and three new scan-weapon scenarios in
`target/audit-scan-weapon-columns-final.log`. All-target Clippy passes in
`target/audit-scan-weapons-clippy.log`. The reference tree is unchanged.

A new full-suite run was started with `cargo test --no-fail-fast`, writing
`target/audit-scan-display-full-suite.log`. It is still running; retain and poll
the existing process rather than restarting it on an observation timeout.
No full green result is claimed. The SCAN/REPORT information block and the
remaining six-section audit scope are still open.


## Shared SCAN/REPORT information rows

`scan_summary.rs` now supplies the reference information block to both commands:
fixed identity columns, six-space indentation, tab separation, coordinates and
heat, full type/movement names, lateral-adjusted heading, VTOL vertical speed,
turret bearing, turret/weapon arcs, visible condition banners and jump heading.
INFO supplements it with torso and observer-relative towing lines. Contact arc
names and vehicle turret geometry are shared with existing consumers. Status
and scan share condition predicates and turret formatting; scans exclude
cockpit-only details. The signed turret boundary now preserves +180, matching
the reference, rather than presenting -180.

Seven new layout scenarios pass in `target/audit-scan-summary-layout-final.log`.
The final regression run passes 230 library, 71 scan and two vehicle-weapon tests
in `target/audit-scan-summary-final.log`. Other affected gunner/vehicle scans,
armor/weapon disclosure and cockpit snapshots passed in
`target/audit-scan-summary.log`. Its old blanket ban on the substring “Weapon”
was corrected to allow the required Weapons Arc line while still rejecting the
weapon table and ammunition. Across the runs, 336 distinct focused tests pass.
All-target Clippy passes in `target/audit-scan-summary-clippy.log`.

The existing full-suite process remains active, writing
`target/audit-scan-display-full-suite.log`. It was started before the information
row changes. Do not restart it because a poll times out, and do not present its
result as full-suite acceptance of those later changes. The six-section audit
objective remains active; the reference tree is unchanged.

### Building surface-entry feedback (2026-09-14)

Added one shared construction-factor notice rule for Mech, ground-vehicle,
VTOL and jump movement. A changed surface hex with a registered entrance reports
`THE NAME has CF of N.`; dropship structures remain silent and concealed
structures use the existing perception roll and XP service. The helper does
not damage or repair construction state. Flight reports now retain any accepted
perception experience diagnostics when combined with the main movement report.

`tests/btech_building_step.rs` covers all five mobile ground chassis, zero and
nonzero CF, duplicate entrance records, ordinary and concealed structures,
guaranteed perception success/failure, dropship exclusion, replay after restart,
and a VTOL actually crossing above the entrance without a notice. Connection
presence must be restored by the host after restart for the perception replay.
The two new tests pass, as do 429 existing building, motion, vehicle-motion and
jump tests (`target/audit-building-step.log` and
`target/audit-building-step-flight.log`). All-target Clippy with warnings denied,
formatting and diff checks pass. No snapshots were regenerated.

The audit's ordinary repair recovery gap was stale: the existing persistence
service and three recovery tests already cover inferred intervals and retained
saved countdowns. The audit now records that evidence. Destroyed-building
rebuild remains an open reference-behavior choice. Interrupted movement and
boundary-transfer entry notifications still need direct acceptance; this change
does not establish every movement-path notification ordering.

The older full scan-display baseline is still live, with 301 completed suites,
2,296 passing tests and no failures observed. That binary predates both the
latest SCAN/REPORT information-row changes and this building-entry change; it
is not a full-current-worktree acceptance result. The broad six-section goal
remains active.

## Transactional global roll history and retirement (2026-09-14)

`World::battle_roll_statistics` now reads live Mech, vehicle and map journals
together with process-local retired history. Histogram merging preflights
overflow. Retirement merges all selected journals before draining any of them,
so repeated retention cannot double-count and an error cannot partially drain
owners. World cloning includes history; persistence deliberately excludes it.

Database planning and delayed wreck retirement retain journals before deleting
simulation state. Maintenance callback replay requires its own final retention
and purge because it starts from the pre-repair world. The new Lua test exposed
a placed-unit purge ordering bug: ordinary purge departure cleared containment
while retaining battlefield identity. Departure now runs leave callbacks first,
retains the journal and forgets the unit identity, then clears containment and
runs move callbacks. This follows the delayed-wreck ordering and keeps failures
inside the existing world/effect transaction.

Tests cover exact aggregation across both unit stores and the map stream,
map replacement, repeated retention, discarded candidates, restart reset,
seven-chassis wreck retirement, database purge for both anatomies, and successful
and aborted Lua cleanup. All 246 library tests pass in
`target/audit-roll-history-unit-final.log`; all 24 wreck/database/Lua tests pass
in `target/audit-roll-history-callbacks-final.log`. Earlier lifecycle logs record
the corrected fixture-link and live departure-order failures. Formatting and
scoped diff checks pass; reference files remain unchanged.

The `+rolls` command is still not registered. Reference-call inventory acceptance
and command admission remain before exposing the report. The full audit goal
remains active; this is not a full-suite or six-subsystem completion claim.

## Wizard roll report exposed (2026-09-14)

Registered `+rolls` through the shared wizard command registry. It returns the
literal world histogram without committing state, ignores trailing arguments
like reference `CS_NO_ARGS`, and rejects switches with the reference reply after
permission checks. Added wizard help and updated command discovery expectations.

The reference inventory's repair, BattleSuit and attached-swarmer-only checks
remain with deferred systems. Supported building missile packets use the shared
cluster resolver. Recovery and seismic generators only supply direct dice. The
current histogram therefore reports explicitly classified generic checks in the
implemented simulation; future rule implementations must preserve that distinction.

All 246 library tests pass in `target/audit-roll-command-unit.log`. All 24 access,
command-registry and roll-report integration tests pass in `target/audit-roll-command.log`;
the final report tests, including aim-preview isolation, pass in
`target/audit-roll-command-final.log`. Tests exercise wizard admission, empty
output outside a cockpit, ignored arguments, switch precedence, actual piloting
rolls on both anatomies, exact report text, direct-dice exclusion, repeated reads
and reset on restart. Formatting and scoped diff checks pass; reference files
remain unchanged. This closes the exposed generic-roll report gap for implemented
rules, not the broader six-subsystem audit.

## Periodic piloting feedback (2026-09-14)

Periodic stability checks now deliver `You make a piloting skill roll!` followed
by the exact target/roll line before damage and fall notifications. Reports
capture the pilot before consequences can evacuate or injure them; absent pilots
use the reference cockpit fallback. Blocked and automatic checks produce no
roll feedback. Ordinary checks, slug unjamming and orbital landing now share one
formatter instead of repeating the strings.

All 246 library tests and 21 periodic/landing/vehicle-unjamming integration tests
pass in `target/audit-piloting-feedback-unit.log` and
`target/audit-piloting-feedback.log`. New biped/quad cases cover success/failure,
assigned and absent pilots, passenger privacy and message ordering. The server
save-failure case verifies the new messages remain unpublished until retry
commits. Formatting and scoped diff checks pass; reference files remain unchanged.

Other control-check callers and MechDebug diagnostics remain open. The audit
also now distinguishes the reference's chargen header declarations from an
executable implementation: no definitions/callers for the named functions were
found in the C/Lua tree. Character generation still needs a behavior contract
and implementation; that qualification does not close the requested workflow.

## Computer failure selection (2026-09-14)

Added a shared typed selector for the missing computer-failure family. It retains
the rare gate, quality catalogue arithmetic, conditional random draws, one-time
six reroll and effect-specific availability gates. Disabled parts and malformed
catalogue ratings leave dice untouched. No targeting, shutdown or recovery state
is changed by selection alone.

All 250 library tests passed in
`target/audit-computer-failure-selection-final.log`. Deterministic draw scripts
exercise all rare branches without relying on a finite random search finding
every outcome. Formatting and diff checks pass; reference files are unchanged.
The new `docs/btech-computer-failures.md` records remaining heartbeat/effect/timer
integration and the reference's encoded-recovery range saturation. This is a
prerequisite implementation, not a claim of live computer-failure parity.

## Unjamming piloting diagnostics (2026-09-14)

Conventional unjamming captures the shared control diagnostic at its position in
the ordered message stream. The direct host action and server heartbeat use the
same publisher, emitting diagnostics before the corresponding cockpit roll even
when several attempts finish together. Rotary unjamming retains its separate
gunnery feedback; empty-ammunition and automatic checks emit no piloting record.

The existing skill/XP acceptance matrix now covers diagnostic counts and order
for success, failure, disconnected crews, tactical units, rotary weapons, empty
bins and prone Mechs. A later XP-channel overflow verifies that an already staged
diagnostic is rolled back with countdown, ammunition, dice, XP, channel history
and recipient output. Restart coverage remains in the same scenario.

All 253 tests passed: 247 library tests and six unjamming integration tests
(`target/audit-unjam-debug-lib.log` and `target/audit-unjam-debug.log`). Formatting
and diff checks pass; reference files remain unchanged. Diagnostics at other
control-check callers, computer failure and the broader audit remain open.

## Pod-removal self-damage feedback (2026-09-14)

Failed iNarc swats retain private impact messages in the pod-removal report,
offset after the swat and self-hit warnings. The host action uses the shared
ordered publisher before its existing injury/fall/casualty consequence traversal.

A new scenario weakens a pod-bearing leg and deliberately misses the pod,
destroying the leg and producing a real protection check. It verifies warning
order, exact pilot-only roll delivery, ordinary passenger messages, direct/Lua
state and output agreement, and callback rollback of damage, dice, pod state and
messages.

All 253 tests passed: 247 library tests and six pod-related movement tests
(`target/audit-pod-feedback-lib.log` and `target/audit-pod-feedback.log`).
Formatting and diff checks pass; the reference tree is unchanged. This closes a
remaining nested impact publisher, not the broader audit or the remaining
control-check diagnostics and computer-failure requirements.

## Heartbeat piloting diagnostics (2026-09-14)

Control checks expose a shared diagnostic formatter containing the skill,
situational modifier, damage penalty and final target. The labels match both
reference control entry points, including the awarding path's `(noxp)` label.
Skipped rolls produce no diagnostic. Heartbeat actions emit this record through
the transactional MechDebugInfo channel before pilot feedback; missing channels
remain silent under the existing channel service.

The heartbeat audience matrix now checks subscriber-only diagnostic delivery,
ordering before cockpit feedback, and full dice/damage/output rollback on channel
counter overflow. Its cockpit expectation includes the independently rolled fall
protection check after failed control. Formatter tests cover both labels and the
silent blocked/automatic cases.

All 255 tests passed: 247 library tests in `target/audit-piloting-debug.log` and
eight heartbeat tests in `target/audit-piloting-debug-final.log`. The first
heartbeat run exposed an outdated single-roll expectation, corrected to include
fall protection. Formatting and diff checks pass; reference files are unchanged.
This wires diagnostics for the initial heartbeat check only. Nested and other
control-check diagnostic callers, computer failure and the broader audit remain
open.

## Vehicle fire and heat feedback (2026-09-14)

Inferno, blast heat, terrain exposure and scheduled-fire reports retain nested
vehicle damage checks at their positions in the notice stream. Their direct host
actions publish that stream, including the fire ticker. Ground movement,
blast-damage aggregation and vehicle-salvo formatting forward the same feedback.
Immediate ignition and later fire pulses share the existing armor/critical rules;
no separate control calculations were added.

A connected VTOL pilot and passenger scenario exercises actual emergency checks
through heat exposure, terrain fire, inferno ignition and a scheduled section
pulse. It compares private messages with retained reports, checks passenger
privacy and ordinary visibility, and verifies deterministic state and output
replay for every path.

All 329 tests passed across the library, vehicle artillery, burning, character,
firing and mines suites (`target/audit-vehicle-heat-feedback-lib.log` and
`target/audit-vehicle-heat-feedback.log`). The expanded scheduled-pulse scenario
also passes in `target/audit-vehicle-heat-feedback-tick.log`. Existing character
and firing tests cover transaction rollback; the new scenario focuses on private
feedback and replay. Formatting and diff checks pass; reference files remain
unchanged. Other control callers, diagnostics, computer failure and the broader
audit requirements remain open.

## Vehicle salvo and launch feedback (2026-09-14)

A shared vehicle-salvo formatter retains each impact's private control messages
at the corresponding packet offset for Mech and vehicle shooters. Direct and
coordinate vehicle launch misloads now expose their internal-damage private
stream, and vehicle falls retain private feedback from their damage impacts.

The new end-to-end firing scenario uses both shooter families against an airborne
VTOL and finds a reproducible engine critical. It checks warning/roll/outcome
order, target-pilot-only delivery, passenger visibility of ordinary messages,
repeatable state and output, and callback rollback. The target template has extra
structure so the engine check can happen before hull destruction; game templates
are unchanged. Fixture setup explicitly handles shutdown for administrative
placement and release of the existing pilot assignment.

All 308 tests passed: 246 library, 38 vehicle firing, six vehicle hex firing,
three vehicle salvo and 15 VTOL crash tests
(`target/audit-vehicle-salvo-feedback-final.log`). Formatting and diff checks
pass; the reference tree is unchanged. Vehicle fire/heat aggregation and the
broader audit requirements remain open.

## VTOL emergency feedback in damage reports (2026-09-14)

Engine-loss emergency checks capture the pilot's roll after the engine-hit
warning and before the landing or forced-descent outcome. Critical, internal,
armor and impact reports retain that private stream, including recursive weapon
and ammunition damage. Direct critical and armor actions use the ordered
publisher. The shared blast packet adapter now handles vehicle private feedback
as well as Mech feedback.

The new connected-pilot scenario verifies both landing outcomes, exact message
order, passenger privacy, and state/report/output replay after save and reload.
Its pilot has an explicit character and skill: connected pilots without profiles
use target 18, unlike the default target six for unassigned crews.

All 290 tests passed: 246 library, seven reactor explosion, 17 vehicle character,
10 vehicle mines and 10 VTOL critical tests. Logs are
`target/audit-vtol-emergency-feedback-lib-final.log`,
`target/audit-vtol-emergency-feedback.log` (the first three integration targets),
and `target/audit-vtol-emergency-feedback-final.log` (corrected VTOL fixture).
Formatting and diff checks pass; reference files are unchanged.

This is not complete vehicle feedback integration. Weapon salvo formatting,
vehicle fire/heat aggregates and other vehicle-impact consumers still need to
forward the newly retained stream. The broader audit also remains open.

## Reactor aggregation feedback (2026-09-14)

Reactor reports retain private packet messages and secondary reactor blasts in
notice order. Reactor-instability prefixes offset the complete private stream;
section exposure and tactical impact reports forward it into their own existing
publishers. Direct reactor actions use ordered publication, shared by Lua and
self-destruct countdowns.

A new scenario forces an instability blast before the main explosion and a
neighboring Mech's real damage-induced control check. It verifies that the safety
warning precedes the roll, compares all private messages with the retained report
without duplicates or passenger disclosure, and checks native/Lua output and
state agreement plus callback rollback. Existing chain tests cover saved replay
and casualty rollback.

All 269 tests passed: 246 library, one reactor attribution, seven reactor
explosion, seven instability and eight self-destruct tests
(`target/audit-reactor-feedback-lib.log` and `target/audit-reactor-feedback.log`).
Formatting and diff checks pass; reference files remain unchanged. Other control
check callers still need review, including VTOL engine-loss emergency checks
through vehicle critical reports. MechDebug, computer failure and the broader
audit requirements remain open.

## Mine aggregation and direct vehicle-fall feedback (2026-09-14)

Mine blast, physical activation and command-detonation reports retain private
packet feedback at the correct offset in their ordinary notices. Ground movement,
jump settlement, prone transitions, Mech and vehicle falls, and deliberate or
automatic VTOL touchdown forward it through the shared movement/fall reports.
Direct mine detonation and command mines use the ordered publisher. The direct
vehicle-fall action also now publishes its private stream; its enclosing movement
and shutdown paths were already covered.

The new mixed-unit command-mine scenario produces actual damage-induced pilot
checks, verifies exact private-message delivery without duplication or passenger
disclosure, compares direct blast feedback, and checks state/report/output replay
after save and reload with connected recipients restored.

All 717 tests passed: 246 library and 471 integration tests across jump, movement,
signed mines, vehicle character effects, vehicle mines and VTOL crashes
(`target/audit-mine-feedback-lib.log` and `target/audit-mine-feedback.log`). Existing
character-action tests cover rollback; the new mine scenario focuses on feedback
and replay. Formatting and diff checks pass, and the reference tree is unchanged.
Reactor aggregation, other control-check callers and the broader audit's remaining
requirements are still open.

## Artillery and scenario packet feedback (2026-09-14)

Shared blast effects retain each Mech packet's private piloting feedback at its
position in the occupant notice stream. Artillery arrival reports offset those
messages after arrival and hit descriptions, and the direct arrival action uses
the ordered publisher. Queued artillery invokes that same action. Wizard packet
damage uses the same packet feedback adapter and publisher across its sequence.

A deterministic damage scenario exercises real damage-induced balance checks,
compares emitted private messages with the retained impact reports, checks
passenger privacy, and verifies native/Lua state and output agreement plus late
callback rollback. It fails if no tested seed produces an actual check.

All 270 tests passed: 246 library, 13 artillery firing, four scenario packets and
seven vehicle artillery tests (`target/audit-blast-feedback-lib.log` and
`target/audit-blast-feedback-final.log`). An initial test setup redundantly tried
to start an already-running fixture; that setup was corrected before the final
run. The new private-feedback scenario covers the shared packet adapter and
wizard publication; artillery suites provide surrounding arrival regression
coverage. Mine and reactor aggregation and broader audit requirements remain
open. Formatting and diff checks pass; the reference tree remains unchanged.

## Vehicle-fall and upward-break feedback (2026-09-14)

Vehicle fall reports retain the pre-injury pilot's protection roll and private
feedback from neighboring units dropped by ice fracture. Shutdown, driving,
obstacle collisions, VTOL crashes and descent, periodic checks, surface breakage
and orbital movement forward the ordered stream. Upward ice breakout and VTOL
crash reports account for their prepended notices when offsetting private rolls;
the direct upward-break action uses the shared ordered publisher.

The mixed-unit shutdown scenario verifies a moving tank breaking ice beneath a
Mech, private roll delivery to both pilots, passenger privacy, native/Lua output
and state agreement, and callback rollback. The upward-break casualty scenario
also verifies breakout-before-roll ordering, pilot-only delivery, saved replay
with restored connections, and rollback when casualty publication fails.

All 246 library tests and 498 integration tests passed across movement, orbital
movement, shutdown, surfaces, vehicle surfaces and VTOL crashes
(`target/audit-vehicle-fall-feedback-lib.log` and
`target/audit-vehicle-fall-feedback.log`). The strengthened upward-break test
passes separately in `target/audit-upward-feedback-verification.log`. Blast and
mine aggregation remain open; these checks do not close the broader audit.

## Pickup and weapon-surface feedback (2026-09-14)

Pickup aggregates retain private feedback from flooding and ice breakage; the
host publishes it in order with attachment and terrain notices. Flooding uses
the shared complete exposure-consequence publisher, including reactor effects,
instead of handling only its fall. Weapon-triggered surface impacts also retain
fracture feedback, and the shared hex-shot formatter offsets it after preceding
shot notices for both launcher families.

Pickup-through-ice acceptance verifies pilot-only protection messages, native/Lua
state and output agreement, and callback rollback. The weapon-surface scenario
now has a running character pilot and a connected passenger on the broken ice;
it verifies private roll delivery alongside native/Lua agreement and rollback.

All 647 tests passed across the library, movement, towing and vehicle hex-firing
suites (`target/audit-pickup-surface-feedback.log`). Formatting and diff checks
pass; the reference tree is unchanged. Vehicle-fall and blast aggregation remain
open, as do the broader audit's other requirements.

## Surface and section-exposure feedback (2026-09-14)

Surface-break and section-exposure reports retain private checks from their
nested falls. Direct surface and flooding actions use the ordered publisher.
Mech falls, critical damage/exposure, prone and DFA immersion, water entry,
ground/jump/orbital movement, and vehicle ground ice breaking preserve those
private insertion positions through their aggregate reports.

The character ice/bridge-collapse acceptance matrix verifies exact protection
roll pairs and passenger privacy alongside failed-evacuation rollback. The
flooded-leg case verifies that the captured pilot still receives private checks
when the subsequent fall floods the cockpit and clears the assignment. A test
initially required a cockpit surface warning before each roll; direct collapse
can produce observer-only warnings, so the corrected assertion preserves the
existing behavior and checks the actual roll pair's order.

Verification: 246 library tests passed in `target/audit-surface-feedback-lib.log`;
67 jump and 361 movement tests passed in `target/audit-surface-feedback.log`;
all 105 surface and 5 vehicle-surface tests passed after the test correction in
`target/audit-surface-feedback-final.log` (784 passing checks). Formatting and
diff checks pass, and the reference tree is unchanged. Vehicle-fall,
weapon-surface, towing/pickup and blast aggregators remain open, along with the
broader audit's other requirements.

## Free-fall and MASC protection feedback (2026-09-14)

Free-fall impact reports now append their protection checks and nested damage
feedback to airborne movement's private stream, after the impact warning.
MASC failure collects fall reports until its ordered warnings and private rolls
have been published, then publishes character and XP consequences under the
same rollback checkpoint.

The free-fall acceptance scenario checks low/high rolled values, passenger
privacy, warning order and saved replay with equivalent connection state. Its
first replay comparison exposed the intentional disconnect-on-load behavior:
connected pilots use their character skill while disconnected pilots use the
default skill. Restoring connection flags in the fixture makes the scenarios
equivalent. The MASC speed-boundary matrix checks protection feedback only for
falls above one MP, with exact rolled values and no passenger disclosure.

Verification: 246 library tests passed in `target/audit-freefall-booster-lib.log`,
11 booster tests passed in `target/audit-freefall-booster-feedback.log`, and all
67 jump tests passed after the replay-fixture correction in
`target/audit-freefall-booster-final.log` (324 passing checks). Formatting and
diff checks pass; the reference tree is unchanged. Surface/flooding aggregators
and the broader audit's remaining requirements remain open.

## Fall protection feedback (2026-09-14)

Fall reports capture the assigned pilot before personal injury can clear crew
assignment. A shared append method emits actual protection-roll feedback before
fall consequences and retains private notices from grouped damage. Automatic,
blocked and combat-safe no-roll paths remain silent; pilotless actual checks use
the cockpit audience.

Direct falls and the main combat/movement aggregators now use that method:
critical balance, physical attacks, charge/DFA, recoil, shutdown, stacking,
stagger, standing/prone, bootleggers, orbital and ordinary landings, jump
settlement, cliff/water movement and thermal falls. Periodic reports retain their
nested fall and gravity-impact private notices through the same ordered publisher.
Free-fall, booster failures and surface/flooding aggregation remain open.

Acceptance extends the fall-protection policy matrix with a connected passenger,
exact leading roll messages and no-roll silence, retaining XP rollback and saved
replay checks. The critical-balance matrix now checks both the initial balance
roll and the separate protection roll instead of assuming a single pair of
messages. Its initial count assertion was the only regression failure; after
correcting that expectation, all 705 library, jump, movement, orbital movement,
shutdown and stacking tests passed in `target/audit-fall-feedback-final.log`.
Formatting and diff checks pass; the reference tree is unchanged.

## Charge and DFA private control feedback (2026-09-14)

Charge and DFA aggregate reports retain private notices from all target and
recoil impacts. Their own control checks capture the pilot before any fall or
injury; DFA miss checks use the same formatter. Mutual charges offset both
attempts into the aggregate stream. Ground movement, jump movement and DFA
completion preserve private positions when they incorporate these reports.
Direct host actions publish through the shared ordered-notice service.

Acceptance checks one-way and mutual charges with two pilots and a connected
passenger, verifying actual balance-roll values after collision warnings without
passenger disclosure. DFA hit/miss acceptance checks the same private delivery
alongside its existing XP publication rollback. Existing movement and jump
scenarios exercise the enclosing dispatch and replay paths. The weapons-hold
comparison helper now adjusts private insertion indices when removing warnings;
the initial failure was a normalization mismatch, not changed damage behavior.

Verification: 246 library and 66 jump tests passed in
`target/audit-charge-dfa-feedback.log`; after the test normalization correction,
361 movement and 21 stacking tests passed in
`target/audit-charge-dfa-feedback-final.log` (694 passing checks across the final
sources). Formatting and diff checks pass. The reference tree remains unchanged.
Other nested feedback paths, control checks, diagnostics and the broader audit's
remaining gameplay requirements are still open.

## Critical-fall collision consequences (2026-09-14)

Airborne critical falls now retain stacking feedback in the enclosing impact's
ordered private stream. Host-capable damage also retains secondary collision
impacts and avoidance falls on the balance report. One balance-consequence
publisher is shared by tactical impacts, salvo groups and fall groups, so nested
character injuries and XP are published once rather than discarded.

Attack capability is retained separately from the primary unit's IC flag. A host
salvo, physical attack, charge or DFA can therefore knock an ordinary unit onto
an in-character neighbor without dropping the secondary casualty capability.
Pure tactical entry points retain their explicit rejection and rollback contract.

A new mixed-unit salvo scenario exercises an airborne gyro loss, collision head
injury, private consciousness messages, deterministic report/state/output replay,
and a fatal secondary injury. Removing the evacuation destination rejects the
whole action without damage, health or output leakage; restoring it permits the
same attack to evacuate the pilot. The initial fatal-case fixture incorrectly
left the secondary pilot a wizard, which exempts evacuation; removing that flag
fixed the test setup without changing production behavior.

Verification: 246 library and 360 movement tests passed in
`target/audit-critical-stacking-final.log`; all 21 stacking tests passed after the
fixture correction in `target/audit-critical-stacking-final-recheck.log` (627
passing checks across the final sources). Formatting and diff checks pass. The
reference tree remains unchanged. This closes the critical-fall stacking gap
recorded below, while the broader audit's other feedback aggregators, control
checks, diagnostics and unsupported gameplay requirements remain open.

## Stacking collision feedback (2026-09-14)

Stacking host actions capture actual avoidance checks and retain private balance
feedback from their nested damage impacts. The shared insertion-offset helper
preserves warning/roll/consequence ordering through movement, jump completion,
orbital landing, thermal falls and shutdown. Bulk map clearing now publishes the
shutdown private stream too. Assigned pilots receive the roll privately;
pilotless checks retain cockpit delivery, and no-roll checks stay silent.

The avoidance acceptance matrix covers ground, jump and fall entries with both
piloting policies, a connected passenger, exact rolled values and failed XP
publication rollback. A damaged-target collision scenario verifies nested
critical-balance privacy and deterministic output/state replay.

Verification: `cargo fmt`, formatting/diff checks and 707 tests passed across the
library, stacking, map clearing, shutdown, jump, orbital movement and movement
suites (`target/audit-stacking-feedback-final.log`). The reference tree is
unchanged. Critical-impact-triggered airborne falls still use the ordinary-only
stacking resolver: private feedback and nested host consequences there remain
open, as do the broader audit's other control checks and diagnostic processing.

## Thermal balance feedback (2026-09-14)

Thermal shutdown balance reports now capture private feedback before shutdown
clears the pilot assignment. Heat-triggered ammunition explosions also retain
their impact feedback. The thermal formatter remaps private insertion positions
around its Computer override diagnostics, then uses the shared ordered publisher.

The shutdown/XP replay test verifies the exact BTH and roll after Computer output,
the captured recipient and privacy, while retaining failed-publication rollback
and restart coverage. All 246 library and 360 movement/combat tests pass in
`target/audit-heat-feedback.log`. Formatting and diff checks pass; the reference
tree remains unchanged. Nested stacking/fall impacts, remaining control callers
and diagnostics are still open within the full six-subsystem objective.

## Periodic stagger roll feedback (2026-09-14)

Stagger reports now capture private control feedback after their damage warning
and before fall messages. All three history modes use the shared ordered host
publisher. Automatic/blocked checks retain no roll output, and recipients are
captured before casualty resolution.

The mode/XP matrix and casualty replay tests verify exact BTH/roll wording,
warning/roll/fall order, privacy, and failed publication rollback. The initial
run passed 246 library and 359 movement/combat tests, with one test-only failure
caused by draining output before an existing replay assertion. That assertion
now reuses the captured messages; all 10 stagger tests pass in
`target/audit-stagger-feedback-final.log`. The initial run is recorded in
`target/audit-stagger-feedback.log`. Formatting and diff checks pass; the
reference tree remains unchanged. Remaining control and nested damage feedback,
diagnostics and the full six-subsystem goal remain open.

## Heavy Gauss recoil feedback (2026-09-14)

Recoil reports now capture the shooter pilot before fall consequences. Unit and
hex shots share one formatter for warning, private control roll and any fall.
The hex firing adapter retains private positions after its shot/observer prefix;
the same report view also carries coordinate misload checks. Stationary shots
continue to skip recoil checks.

The direct-shot matrix verifies exact BTH/roll output, stationary silence and
failed XP-publication rollback. The failed hex-recoil scenario verifies native/
Lua agreement, passenger privacy, warning/roll/fall order and callback rollback.
All 246 library and 360 movement/combat tests pass in
`target/audit-recoil-feedback-unit.log` and `target/audit-recoil-feedback-motion.log`.
Formatting and diff checks pass; the reference tree remains unchanged. Remaining
nested damage, other control callers and diagnostics remain open within the full
six-subsystem objective.

## Launch-misload critical feedback (2026-09-14)

The shared launch-failure formatter carries private impact checks after its
failure warning and observer messages. Mech unit shots and mixed-anatomy hex
and artillery launches obtain those notices from their existing impact reports.
Direct shot publication also retains misload checks. Vehicle-only misloads have
no Mech balance stream and remain silent there.

A rapid-fire AC/2 leg misload verifies actual critical-balance feedback, warning
order, passenger privacy and complete Lua callback rollback. All 246 library tests
pass in `target/audit-misload-feedback-unit.log`; 360 movement/combat, 13 artillery
firing, 6 vehicle artillery-launch and 6 vehicle hex-fire tests pass in
`target/audit-misload-feedback-integration.log`. Formatting and diff checks pass;
the reference tree remains unchanged. Other nested damage and control-check
feedback remain open within the six-subsystem objective.

## Swarm critical feedback (2026-09-14)

Swarm reports now retain private damage checks across their target hops. Each
hop offsets its salvo feedback after the preceding flight notices, and the outer
target-salvo adapter carries the combined positions into shared shot publication.
The unused ordinary-only target-salvo wrapper was removed so callers explicitly
carry the private stream.

A seeded secondary-target scenario exercises both Mech and vehicle launchers,
verifies the target pilot receives the roll in order, compares complete native
and Lua output/state, and checks aborted-callback rollback. All 246 library and
12 Swarm tests pass in `target/audit-swarm-feedback.log`. Formatting and diff
checks pass; the reference tree remains unchanged. Launch-misload and other
nested damage feedback remain open within the full six-subsystem goal.

## Shot-level critical feedback (2026-09-14)

Mech target salvos retain private balance notices while assembling woods,
inferno and damage-group output. Mech and vehicle shot reports carry those
positions through their own prefixes, and the shared firing formatter offsets
them after shot and observer announcements. Configured native/Lua firing and
direct shot actions use the ordered publisher. Ordinary read-only notice views
remain available without broadening the private audience.

A seeded native firing scenario verifies target-only critical balance feedback,
then checks identical Lua state/output and complete rollback after an aborted
callback. All 246 library tests pass in `target/audit-shot-feedback-unit.log`;
14 gunner, 359 movement/combat and 37 vehicle firing tests pass in
`target/audit-shot-feedback-integration.log`. The previously failing unjamming
server test passes in this run as well. Formatting and diff checks pass; the
reference tree remains unchanged. Swarm, launch-misload and other nested damage
aggregation remain open, along with the rest of the six-subsystem goal.

## Critical-impact feedback capture (2026-09-14)

Critical balance reports now capture the assigned pilot before fall consequences.
The impact resolver records private roll notices at the point before its fall
messages. Tactical impacts and salvo groups retain those notices; physical
attacks merge them, and direct ammunition-explosion and group publication use
the shared ordered publisher. This is partial integration: broader shot notice
assembly and other nested impact aggregators still need to carry the new field.

The actuator/gyro matrix verifies exact roll text and recipient, insertion before
falls, forced-fall silence and saved replay. All 246 library tests pass in
`target/audit-impact-feedback-unit.log`; the movement/combat run passed 357 tests
and failed `unjam_character_server_tick_retries_failed_commit` at its saved-state
comparison (`target/audit-impact-feedback-motion.log`). That test passed in an
isolated rerun (`target/audit-impact-feedback-unjam-recheck.log`); its concurrent
setup/tick timing remains an unresolved test concern, not a clean suite baseline.
Formatting and diff checks pass; the reference tree remains unchanged. The full
six-subsystem objective remains active.

## Physical-attack balance feedback (2026-09-14)

Kick, trip and missed-mace checks capture the balancing unit's pilot before fall
consequences. Single attacks use the shared ordered publisher; arm sequences
merge the private insertion positions alongside their ordinary notices. Missing
pilots use the existing cockpit fallback, while automatic and blocked checks
remain silent. Critical-damage balance inside impact cascades is still separate
open work.

The missed-mace success/failure test now executes the host arm action and verifies
warning/roll ordering, exact BTH and dice text, and passenger privacy. All 246
library and 358 movement/combat tests pass in
`target/audit-physical-feedback-unit.log` and
`target/audit-physical-feedback-motion.log`. Formatting and diff checks pass;
the reference tree remains unchanged. Remaining control feedback, diagnostics
and the broader six-subsystem audit goal remain open.

## Jump launch roll feedback (2026-09-14)

The shared conventional/DFA launch action now retains private stagger-roll
feedback after the coordination warning and publishes it through the ordered
notice service. Failed checks, successful launches and subsequent private
destination rejections keep their existing transaction semantics. Attempts below
the stagger threshold remain silent and do not consume extra dice.

The biped/quad native/Lua matrix verifies ordered roll text for success and
failure, saved replay and aborted callbacks. Rejected-request tests verify that
passengers receive the warning but neither the roll nor the private rejection.
All 246 library, 66 jump and 3 stagger-launch tests pass in
`target/audit-launch-feedback.log`. Formatting and diff checks pass; the reference
tree remains unchanged. Other control feedback and diagnostics remain open within
the broader audit goal.

## Jump completion roll feedback (2026-09-14)

Jump elevation avoidance and landing checks for stagger and damaged legs/gyro
now capture shared private feedback after their warnings. Landing-local reports
retain those notices through ordinary completion, obstacle rollback, lost-thrust
resolution and the early-landing host adapter. Existing admission and automatic
no-roll behavior remain owned by the control rules.

The character landing matrix verifies the one-check ordinary landing and
two-check obstacle/gear sequence, exact rolled sums, private recipients, XP
publication failure rollback, retry and saved replay. All 246 library and 66 jump
tests pass in `target/audit-jump-roll-feedback-final.log`. Formatting and diff
checks pass; the reference tree remains unchanged. Launch-time stagger feedback,
combat balance, other control callers and diagnostics remain open within the
broader six-subsystem goal.

## VTOL terrain roll feedback (2026-09-14)

Forest and elevation obstacle reports now retain the shared pilot feedback at
the point after the terrain warning and before landing/crash consequences. The
flight update preserves those positions and recipients through movement report
aggregation. Pilotless elevation avoidance and disconnected-pilot forest entry
remain silent because those paths do not roll.

The existing forest/elevation matrices now verify exact roll wording and rolled
sum, warning-before-roll ordering, and pilot-only delivery with a connected
passenger. Their deterministic success/failure and saved replay checks remain.
All 311 tests pass in `target/audit-vtol-feedback.log`: 246 library, 11 hiding,
5 map wrapping, 11 VTOL controls, 15 crash and 23 flight tests. Formatting and diff
checks pass; the reference tree remains unchanged.

Other control-call sites remain to be integrated, including jump completion,
physical/combat balance, stacking, stagger and heat consequences. MechDebug and
computer-failure processing also remain open within the broader audit goal.

## Ground slope and water roll feedback (2026-09-14)

Cliff and reverse-slope avoidance retain their actual check for both Mechs and
vehicles. Auto-fall and pilotless cliff exemptions remain silent. Mech water
entry captures its actual roll after the water warning. A shared insertion-offset
helper carries private notices through nested ground segments, interrupted jump
settlement and movement reports without copying the control rules.

The library, jump and Mech movement suites pass 246, 66 and 358 tests in
`target/audit-slope-feedback-final.log`. The Mech water/reverse-slope matrix now
checks one ordered pair of roll messages, XP publication rollback and successful
retry. Vehicle collision assertions distinguish actual rolls from recovery-blocked
checks; an initial assertion incorrectly required feedback for an unconscious
crew and was corrected using the shared control service on a detached probe.
All 22 vehicle-driving tests pass in `target/audit-slope-feedback-vehicle-final.log`.
Formatting and diff checks pass, and the reference tree remains unchanged.
VTOL terrain feedback and remaining control callers still require integration.
The broader six-subsystem goal remains active.

## Vehicle terrain roll feedback (2026-09-14)

The shared terrain control result retains its pilot and actual check until the
caller captures feedback after its introductory warning. Pilotless exemptions
retain no check. Vehicle water, tree/rock and bridge-underside avoidance use this
service and the existing ordered publisher. Water-entry reports and the outer
movement phase now use `MovementReport::extend`, preserving private insertion
positions instead of discarding the private notices during aggregation.

All 22 vehicle-driving tests and 246 library tests pass in
`target/audit-terrain-feedback.log` and `target/audit-terrain-feedback-unit.log`.
The aggregation regression run also passes all 358 Mech movement/combat and
23 VTOL flight tests in `target/audit-terrain-feedback-motion.log`.
The water-entry matrix explicitly verifies warning, private roll and outcome
ordering for tracked/wheeled success and failure, and passenger privacy, while
retaining replay and persistence checks. Formatting and diff checks pass; the
reference tree remains unchanged. Reverse slopes, cliffs, Mech water entry and
VTOL terrain control still need feedback integration. This does not close the
broader audit goal.

## Bootlegger roll feedback (2026-09-14)

Bootlegger attempts now capture the shared two-line piloting feedback before
the pivot or failed-maneuver notices, matching the reference call order. The
detached report retains the pilot recipient and insertion position, and the
shared ordered publisher sends the roll only to that pilot. Cockpit passengers
still receive the maneuver outcome. No extra roll or combat rule is introduced.

The success/failure replay test now checks exact roll text, output order,
passenger privacy and removal of staged output on an aborted Lua callback.
Both bootlegger integration tests and all 246 library tests pass in
`target/audit-bootlegger-feedback.log` and
`target/audit-bootlegger-feedback-unit.log`. Formatting and diff checks pass;
the reference tree remains unchanged. Terrain-control feedback, diagnostics
and the remaining six-subsystem audit requirements remain open.

## Ordered control-action feedback (2026-09-14)

Standing, controlled prone drops and manual early jump landing now capture
private piloting feedback before their consequences. `BattlePilotNotice` retains
the recipient and insertion point, and one shared publisher interleaves it with
ordinary cockpit/observer notices. Movement and orbital landing use that same
publisher. Automatic standing and slow drops do not fabricate roll messages.

All 246 library tests pass in `target/audit-control-feedback-unit.log`. The jump
and orbital suites pass 66 and eight tests in `target/audit-control-feedback-actions.log`.
That grouped run also recorded a prone-fixture failure after the test changed
the pilot's connection state and thus its skill target. Preserving that state
corrected the fixture: all seven prone tests pass in
`target/audit-control-feedback-prone-final.log`. All nine standing tests pass in
`target/audit-control-feedback-stand.log`.

Coverage includes exact native/Lua order, successful and failed checks, automatic
silence, cockpit passenger and outside observer privacy, restart/replay and
callback rollback. The early-landing server test explicitly verifies new feedback
is suppressed by a failed save and delivered on successful retry. Formatting and
scoped diff checks pass; reference files remain unchanged. Other control-check
callers and debug-channel diagnostics remain open within the broader audit.

### Building entry boundaries and completed baseline (2026-09-14)

The previous goal turn made code and verification progress. This continuation
found and fixed a real missing path: boundary exits occur after the low-level
movement report, so successful exits now run the shared building-step rule at
the accepted destination. Denied exits retain their stopped state. The host
movement checkpoint also covers the new notice, perception roll and XP.

The existing boundary scenario now checks a single CF notice for surface units
and none for airborne VTOLs, across all four headings and all six mobile chassis
families. Its callback rejection still restores all state and output. A new
five-chassis cliff matrix verifies no notice for an unentered uphill building
and exactly one for an accepted downhill fall, including zero CF. The complete
building entry-action, route and step suites pass 21 tests in
`target/audit-building-step-acceptance.log`; all-target Clippy, formatting and
diff checks also pass. The named ordinary step-on-base gap is addressed; this
is not broad movement-system acceptance.

The full scan-display baseline finished: 335 suites, 2,585 passing tests and one
failure in `foundation::tcp_account_administration_and_restart`. Its byte-for-
byte live SQLite comparison races with the independent turn-clock writer. The
assertion now compares every saved game-state field while allowing only that
clock to advance, matching the live-help test's established contract. The
account test passes in `target/audit-account-report-clock.log`; no runtime
account code changed.

A fresh full-current-worktree run is underway in
`target/audit-building-entry-full-suite.log`. The completed baseline predates
SCAN/REPORT information rows and the building-step changes, so neither it nor
the single corrected account test proves this new full run green. The reference
tree remains unchanged and the six-section goal remains active.

### Shared unit shot counters (2026-09-14)

The previous turn made verified code progress on building-boundary feedback.
This continuation checked the audit's character-counter claim against current
reference call sites. The named character values exist in the catalogue, but
reference combat updates unit runtime counters instead. No per-character
increments were invented. The audit now distinguishes that fact from the real
unit-statistics gaps.

Added shared signed shot-counter storage to Mechs and vehicles, including
validated snapshot restoration and native/Lua unit field inspection and edits.
Direct-unit launches record one fired attack and one broadcast-classified hit
or miss. Coordinate/observer fire, failed Streak locks and failed loaders do not
count; ordinary out-of-range launches count as misses. The shared broadcast
threshold retains the configured near-miss band independently of missile or
beacon material admission. Counter overflow aborts the complete firing
transaction. Vehicle firing now receives the existing `ShotTarget` coordinate
intent before resolving effects instead of adding that intent only to the
finished report.

Five new integration tests cover all seven chassis, native/Lua agreement,
callback and overflow rollback, signed edits, restart, both hit outcomes,
occupied coordinates, failed Streak locks and out-of-range attacks. One focused
unit test checks the broadcast classification bands. The affected checks pass
479 distinct tests: 358 motion/firing, 42 fields, 37 vehicle fire, 13 artillery,
14 gunner fire, one gunner TIC, two TIC, six vehicle hex fire, five new counter
integration tests and one classification unit test. The original motion run
passed 356 tests and identified two manual-composition expectations that omitted
the new counters; both were updated with explicit expected counts and pass
separately. All their ammo, dice, heat and damage comparisons remain intact.

Evidence: `target/audit-shot-counters-regressions.log`,
`target/audit-shot-counters-firing.log`,
`target/audit-shot-counters-composition.log`,
`target/audit-shot-counters-remaining.log`, and
`target/audit-shot-counters-unit.log`. All-target Clippy with warnings denied,
formatting and diff checks pass. See `docs/btech-unit-statistics.md` for the
contract and remaining damage/kill accounting work.

The building-entry full baseline remains live (PID 3113274 at the latest
process check); it predates these shot-counter changes and must not be described
as full-current acceptance. The reference tree is unchanged. Unit damage and
kill attribution, character generation and the other outstanding six-section
requirements remain open; the broad goal is active.


## Damage accounting and attribution acceptance (2026-09-14)

Confirmed the completed damage-counter implementation and its final manual
composition checks. The two corrected motion tests pass, and all-target Clippy
with warnings denied completed successfully in
`target/audit-damage-counters-clippy.log`. The previous building-entry full
baseline is now terminal: 337 suites, 2,596 passing tests, zero failures in
`target/audit-building-entry-full-suite.log`. That baseline predates shot and
damage counters and is not full-current acceptance.

Extended acceptance for three consequence paths. Kick boundary tests assert
exact attacker/target counters for misses, full hits, both glancing policies
and balance falls. The direct-shot composition test now independently requires
205 damage for its five-point laser plus 200-point ammunition explosion, so
matching omissions in both composition paths cannot conceal missing accounting.
The reactor blast matrix verifies no inflicted credit, zero received damage for
excluded recipients and positive received damage across all seven chassis;
existing rollback and restart comparisons cover those counters too.

Eight focused tests pass: the three strengthened scenarios and all five damage
counter integration tests. Evidence is in `target/audit-damage-attribution.log`
and `target/audit-damage-nested-attribution.log`; the first log retains an initial
test diagnostic-format compile error, corrected before the passing reactor run.
Formatting and diff checks pass. No reference files were modified.

Kill accounting, other physical attacks, vehicle ammunition cascades and delayed
blast attribution remain open, alongside the other outstanding requirements in
the six requested audit sections. The broad goal remains active.


## Kill transition accounting (2026-09-14)

Added shared signed `units_killed` storage, restoration and field controls for
Mechs and vehicles. Material damage and lethal equipment/crew critical owners
sample destruction immediately around their own mutation and award a checked
increment before secondary consequences. This avoids a shadow death ledger and
prevents nested packets from duplicating a kill. Self/environmental events do
not award a kill; overflow rejects the enclosing candidate transaction.

Two new integration tests cover all seven supported attacker/target families,
native/Lua agreement, signed bounds, restart, environmental destruction and
callback/overflow rollback. The existing direct-shot composition test now
expects exactly one kill from its seeded ammunition explosion. The initial
fragile-quad fixture caused a lethal balance fall before shot transfer, correctly
receiving no attacker credit; the lethal-shot matrix now isolates center-torso
and hull destruction instead of conflating the causes.

Focused verification passes 67 tests: five damage-counter tests, two kill-counter
tests, one direct-shot composition, six impact tests, 42 unit-field tests, three
vehicle critical-resolution tests and eight vehicle internal-damage tests.
Logs: `target/audit-kill-counters-initial.log`,
`target/audit-kill-counters-final.log`,
`target/audit-kill-composition-final.log`, and
`target/audit-kill-regressions.log`. All-target Clippy with warnings denied,
formatting and diff checks pass. The reference tree is unchanged.

A new full-current suite is running under session 66765, with output in
`target/audit-unit-statistics-full-suite.log`; completion has not been established.
Do not restart it solely on an observation timeout. Damage-induced vacuum/flooding
attribution, special reactor events and exhaustive lethal crew/critical branch
acceptance remain open, together with the broader six-section audit goal.


## Environmental kill attribution in progress (2026-09-14)

Mech vacuum exposure now forwards its attacker into the shared section-exposure
service, which records the immediate destruction transition before secondary
falls or reactor effects. Water flooding still supplies no external attacker.
Reference inspection established that `mech_flood_section` deliberately calls
`mech_parts_destroy(mech, mech, ...)`, including after combat damage; vacuum's
`mech_location_breach` forwards the attacker's identity instead. An initial
attempt to propagate the shot author into water flooding was removed after this
caller-level check.

Added a biped/quad, water/vacuum shot matrix that requires surviving head
structure, environmental destruction, the distinct expected kill totals,
restart and callback rollback. Verification is pending: session 53330 runs
`btech_mech_vacuum` and `btech_kill_counters`, logging to
`target/audit-exposure-kill.log`. It is waiting for the active full-suite build
(session 66765, `target/audit-unit-statistics-full-suite.log`). Both handles were
revalidated live; the build has active compiler processes. Do not restart either
solely because observation times out. The full run was launched before these
exposure changes and cannot establish acceptance of them.

Reference `mech_parts_destroy` also establishes a separate outstanding defect:
ground-vehicle vacuum breaches are fatal, but Rust currently only disables
section equipment. Correct mortality and attribution together; the VTOL branch
is distinct. The audit records this finding. Formatting and diff checks pass;
the new exposure test has not yet completed. The broader goal remains active.


## Exposure attribution verified; vehicle mortality claim withdrawn (2026-09-14)

Completed the Mech exposure change and its biped/quad water/vacuum shot matrix.
Vacuum head exposure credits the attacking unit before secondary effects; water
flooding remains self-attributed even when a shot opens the armor. Tests require
surviving head structure, the correct distinct totals, restart and callback
rollback. The initial fixture attempted to relocate running units and was
corrected to use shut-down placement followed by restored running state.

The previous entry's claimed ground-vehicle vacuum mortality defect was wrong.
Inspection of the complete `mech_parts_destroy` control flow confirms the vehicle
branch returns immediately after disabling equipment. Its later ground-vehicle
death branch is unreachable. An experimental mortality change and its altered
expectations were removed. The original equipment-only implementation remains;
a new five-family shot matrix explicitly verifies surviving hull/crew, zero kill
credit, restart, callback rollback and successful nonlethal firing with a full
kill counter. The existing delivery document had correctly recorded this early
return; its behavior assessment remains valid.

All 30 focused tests pass in `target/audit-exposure-corrected.log`: four kill
counter tests, eight Mech vacuum tests, ten vehicle armor-damage tests and eight
vehicle internal-damage tests. Superseded logs `audit-ground-vacuum.log` and
`audit-vacuum-final.log` are not evidence for the final vehicle behavior. All-target Clippy with warnings denied, formatting and diff checks pass;
Clippy output is in `target/audit-exposure-corrected-clippy.log`.

The full baseline remains live under session 66765. At the latest check, 99
suites had completed with 678 passing tests and zero failures. It predates the
exposure changes and is not full-current acceptance. No reference files changed.
Special reactor kill events and exhaustive lethal crew/critical acceptance remain
open, along with the broader six-section audit goal.


## Initial inferno attribution (2026-09-14)

The vehicle missile path now forwards its existing shooter identity through
initial inferno effects. Standard heat explosions award one kill; advanced
initial section-fire damage retains damage and critical-death attribution.
Scheduled pulses and ambient/blast heat remain self-attributed. The shared
burn-damage service uses the existing attributed armor entry, and no persistent
last-attacker state was added to fires. Reference evidence is the complete
`mech_heat_effect_apply`, `vehicle_fire_start` and `vehicle_burn_event` paths.
Special reactor attribution remains a separate open item.

Added a 20-scenario matrix spanning Mech/vehicle shooters, all five vehicle
families and both fire policies. It verifies native/Lua agreement, independent
initial packet totals, standard heat kills, the seeded advanced VTOL rotor
power-plant catastrophe (one kill), stationary jelly, restart, callback rollback
and damage/kill overflow rollback. Surviving ground fires advance through a
60-second pulse after restart: target damage increases but shooter totals do not.
The fixture seeds the heat roll after the missile cluster roll; an earlier
one-roll assumption was corrected.

All 13 focused tests pass: the new matrix, ten vehicle-burning regressions and
two inferno-ammunition tests. Evidence is in
`target/audit-inferno-attribution-final.log` and
`target/audit-inferno-attribution-regressions.log`. All-target Clippy with warnings
denied, formatting and diff checks pass (`target/audit-inferno-attribution-clippy.log`).

The full baseline remains live under session 66765. Its latest summary showed
218 completed suites, 1,812 passing tests and zero failures. It predates the
exposure and inferno changes and is not acceptance for those changes. The broad
six-section goal remains active; reactor-specific and remaining attribution
acceptance, character generation and other outstanding audit requirements are
not closed by this increment.


## Reactor attribution acceptance (2026-09-14)

Verified the complete reference reactor entry and section-destruction paths.
The existing Rust material/exposure event records the initiating destruction
before reactor detonation. A second reactor-level update would duplicate that
kill, while radial blast packets intentionally retain self attribution.

Added `tests/btech_reactor_attribution.rs`: Mech and vehicle shooters each destroy
biped and quad reactors, whose blast also destroys a fragile neighboring vehicle.
The shooter receives exactly one kill; the reactor and neighbor receive none.
Native/Lua, callback rollback and restart agree. The standalone reactor matrix
now explicitly checks zero kill credit for all supported recipient families.
No additional reactor mutation or attribution ledger was needed.

All 14 distinct focused tests pass: the new attribution matrix, six reactor
explosion tests and seven instability tests. The strengthened standalone test
also passes separately. Evidence: `target/audit-reactor-attribution.log`,
`target/audit-reactor-self-attribution.log` and
`target/audit-reactor-attribution-clippy.log`. All-target Clippy with warnings
denied, formatting and diff checks pass.

The full baseline is still active under session 66765. The latest summary has
319 completed suites, 2,481 passing tests and zero failures; it predates exposure
and inferno changes. Continue observing the existing run. Remaining lethal
critical/crew branch acceptance and the broader audit requirements remain open.
The character-command inventory is the next implementation area: reference
`do_charclear` is in `character/character_health.c`, `+show` dispatch is in
`commands/btech.c`, `btcharlist` is in `character/character_battle_value.c`, and
`btthreshold_func` is in `character/character_persistence.c`.


## Character-list Lua API and completed baseline (2026-09-14)

Added `btech.character.list(kind, player?)` using the existing skill and advantage
catalogs plus the five fixed attribute names in reference order. Unique category
prefixes are accepted. Optional targets resolve by player ID, name, account alias
or #dbref. The reference learned-value condition filters skills only: attributes
and advantages remain complete even with a target. Explicit nil, invalid targets,
ambiguous categories and wrong arity are rejected. No character is initialized
and no XP, recovery or dice state changes during inspection.

The new integration test verifies catalog order, raw levels, low-bit XP,
earned-level XP, last-use-only omission, absent profiles, player names/dbrefs,
detached output, invalid arguments, unchanged state/outbox and restart. Together
with 14 existing character tests, all 15 focused tests pass. Evidence:
`target/audit-character-list-final.log` and
`target/audit-character-list-names-final.log`. All-target Clippy with warnings
denied passes (`target/audit-character-list-clippy.log`). LuaLS documentation is
updated in the game and test fixture. The audit now recognizes the existing
threshold Lua API and leaves `+charclear` and `+show` explicitly open.

The full baseline session 66765 completed successfully and was reaped: 340 suites,
2,611 passed, zero failed (`target/audit-unit-statistics-full-suite.log`). It was
launched before the exposure, inferno and character-list changes and must not be
reported as full-current acceptance. No full baseline is currently running.
The broad six-section goal remains active.

## Wizard character catalogs (2026-09-14)

Added native wizard-only `+show` for all six reference categories. Character
reports use the shared skill and advantage catalogs and shared attribute names,
with exact headings, three 24-column cells, subsequent-row indentation and
footer totals. The general-value names and special-object field inventory are
presentation metadata. All 115 special fields retain reference order and kind
numbers; this does not claim completion of their independent getter/setter audits.
The equals-separated second argument is ignored. The reference help's `char_`
prefixes remain advertised but rejected, matching the executable lookup.

Reading `listmatch` itself established exact case-insensitive matching, not
prefix matching. Corrected the preceding character-list API implementation,
tests and current documentation to reject abbreviated categories. The earlier
progress entry's prefix claim is withdrawn. Existing player selection, skill-only
filtering, detached results and read-only semantics remain in place.

Three focused tests pass: native catalogs/admission/restart, Lua character lists,
and literal character-report spacing. Logs: `target/audit-character-show-final.log`
and `target/audit-character-show-unit.log`. The first admission test accidentally
used the fixture's second wizard as an ordinary player; its corrected setup
explicitly removes Wizard before checking permission denial. No production
permission changes were needed. The reference tree remains unchanged.
Formatting, diff checks and all-target Clippy with warnings denied also pass
(`target/audit-character-show-clippy.log`). The full suite was not repeated for
this change; the completed baseline remains separate evidence. `+charclear` and
the broader active audit requirements remain open.

## Character reset command (2026-09-14)

Implemented wizard-only `+charclear` through a shared, authorized, atomic domain
reset. Reference `do_charclear` frees fixed and variable character state, whose
subsequent reads default attributes to one and health to zero. Rust represents
those effective defaults explicitly so existing recovery validation and pilot
injury callers continue to work. Skills, advantages, XP and last-use timestamps
are removed. Unit pilot assignments, damage, injuries and wreck state remain
unchanged. Pending recovery keeps its countdown and private dice; cached pain
resistance and Toughness are cleared so the next check sees the cleared stats.
No new recovery stream is created and no immediate consciousness roll occurs.

The native command preserves the empty/unknown/success messages and shared player
name/alias/dbref lookup. Both command and domain entry enforce wizard authority.
Repeated clearing is idempotent. Tests cover invalid and denied requests, absent
profiles, active recovery, persistence, an independent next-roll oracle, and
subsequent injury of assigned Mech and vehicle pilots with default Build.

All 18 distinct focused tests pass across character arithmetic, clear, list and
show suites (`target/audit-character-clear.log` and
`target/audit-character-clear-pilot.log`). Formatting, diff checks and all-target
Clippy with warnings denied pass (`target/audit-character-clear-clippy.log`).
The reference tree is unchanged. The broader goal remains open; this does not
close character generation, personal combat or other listed subsystem gaps.

## Evacuation XP selection and signed balances (2026-09-14)

The audit's missing-caller claim was stale: `evacuation.rs` already invokes the
shared retention operation after successful relocation, using IC configuration,
`xploss < 1000`, and wizard exemptions. Reference `contents_teleport` and
`mech_contents_kill_if_in_character` confirm those gates. MW autoejection remains
an independent open requirement.

Corrected retention's selected state. The reference reduction loops over its
catalog, but `character_state_adapter.c` retrieves/stores variable XP only for
skills, advantages and Lives. Rust now reduces those entries, leaves custom and
other unselected entries untouched, and clears signed-negative XP bit patterns.
This removes an erroneous unknown-XP failure that could abort crew evacuation.
Skill bonuses still use shared current thresholds; non-skill selected entries
have the reference default zero threshold. Base values and use timestamps survive.
Reference custom threshold edits for non-skill values are not established by this
change; the existing threshold API remains skill-scoped.

All 37 focused tests pass: 15 character, five crew and 17 vehicle-character
(`target/audit-evacuation-xp-final.log`, `target/audit-evacuation-xp-vehicles.log`).
The new matrix checks all 22 advantages plus Lives at five retention fractions,
and signed-negative values across every skill and advantage. A lethal-head
casualty verifies reduced advantage/Lives XP, cleared negative skill XP, preserved
custom XP, and complete rollback on failed departure. Manual evacuation verifies
wizard exemption, callback rollback and restart. Formatting and diff checks pass;
reference files remain unchanged. The broader goal is still open.
All-target Clippy with warnings denied also passes
(`target/audit-evacuation-xp-clippy.log`). No full-suite process was started here.

## Startup pilot health and explicit tactical death (2026-09-14; running)

Implemented the shared `fix_pilotdamage` startup projection after successful
admission for both chassis stores. The reference sets the cockpit scalar from
(bruise + lethal)/(2*Build), with divisor 10 when doubled Build is outside 1–100.
The setter clamps to signed-char bounds. High initial counts do not themselves
kill crew: a new persisted `pilot_killed` event result replaces numeric >=6 as
the tactical death predicate. Actual injury records death and performs ordinary
cleanup; IC injury retains its separate health-based death condition.

Focused startup, recovery, unit-field, Mech/vehicle power and vehicle-character
suites passed 78 tests (`target/audit-startup-health-final.log`). The new matrix
covers seven chassis times two character modes, native/Lua agreement, callback
rollback, unchanged health/recovery/dice, live six-count startup, subsequent
injury and restart. Earlier failures were corrected: healthy IC startup keeps
an absent optional status; the fixture clears an invalid running target lock
before pretending the unit is powered off.

After that focused run, verified `clamp_int_to_char` and applied a shared 127
bound to startup and subsequent tactical/IC counts. Tactical counts no longer
cap at six. Updated explicit overflowing-injury and nested-reactor test oracles
(127 and eight respectively); full verification is pending.

LIVE full-suite process: exec session 90424, `cargo test --no-fail-fast`, output
`target/audit-startup-health-full.log`. It was started in this turn and has not
been reaped. Inspect/poll this handle before starting another full run. Session
28600 (78 focused tests) is reaped with exit zero. No Clippy run for this change
yet. Formatting/diff checks pass and the reference tree is unchanged. The full
active audit goal remains open.

### Startup verification continuation (2026-09-14)

Full-suite session 90424 is confirmed live, still compiling; active rustc/linker
processes were observed. It has not been restarted. All-target Clippy is queued
behind the same build lock in session 76849, with output in
`target/audit-startup-health-clippy.log`; it is not yet a passing check.
No production changes were made during this continuation.

Next output gap characterized while waiting: `mech_recovery_event` calls
`handlemwconc(mech, 0)`, which privately reports the attempt and target/roll;
success then tells occupants "The pilot regains consciousness!". Rust currently
returns only a short player-owned success/failure notice. The existing
`BattleCharacterNotice`/`notify_character` path can carry the already-resolved
check, avoiding extra dice. Player-owned recovery after leaving a cockpit is an
existing tested lifecycle; preserve that while matching occupied-cockpit output.
Also inspect IC `pilotdam` reads: startup sets both count representations, but
subsequent character injury currently updates only `character_pilot.injuries`.

## Recovery feedback (2026-09-14)

Added typed check/audience information to `BattleCharacterNotice`, replacing its
single static-text field. The event captures its existing target, roll, outcome,
assigned live cockpit and blindness. Shared output publishes reference private
attempt/roll messages and a successful occupant announcement. Failed occupied
checks have no public output; blinded cockpits have no feedback. Empty crews
announce successful recovery only. Released pilots retain the existing
player-owned recovery lifecycle and private outcome message.

All 12 distinct focused tests pass: four recovery, four empty-crew, two character
clear and two recovery-output unit tests. The latter include real notification
fan-out to pilot/passenger and verify that rolls remain private. The seven-chassis
matrix independently predicts the one consumed roll, success/failure, blindness,
released-cockpit behavior and next timer. Existing server save-failure replay
passes. Logs: `target/audit-recovery-feedback.log`,
`target/audit-recovery-feedback-final.log`, `target/audit-recovery-output-unit.log`.
All-target Clippy passes in `target/audit-recovery-feedback-clippy.log`.
Initial test-only compile errors (Document plain-method spelling and a path
module inside a Tokio macro) were fixed; final logs above supersede them.

The full pilot-health baseline remains live in session 90424, now executing tests
without observed failures. Its compilation began before this feedback change;
its results must not be called a full-suite acceptance of recovery feedback.
Clippy session 76849 failed on the early test-only spelling and is reaped;
39815 is the corrected successful Clippy run and is reaped. Focused sessions
73665, 75597 and 15725 are reaped with exit zero. No second full suite was started.

## Shared cockpit count writes (2026-09-14; movement checks running)

Fixed `pilotdam` disagreement after IC injuries by routing startup, character
injury, tactical injury, terminal carryover and administrative edits through one
chassis adapter, `pilot_health::set_count`. It synchronizes the cockpit scalar
and optional character report. Confirmed tactical/character deaths remain
independent. The seven-chassis startup matrix now checks field inspection through
native/Lua, cockpit status, persistence, and clearing after pilot release.

Restored saved-count validation at the reference signed-byte maximum 127.
Contact and vehicle-mass wreck fixtures now explicitly set `pilot_killed`; a count
of six may represent a living pilot after startup. Character injury fixtures now
expect the shared count rather than a permanently zero tactical field.
Corrected a previous test-oracle mistake: the simple reactor casualty has two
preexisting hits plus four reactor hits, totaling six. Only the distinct nested
four-plus-four case totals eight. The earlier progress wording suggesting both
should be eight is withdrawn.

Final focused acceptance passes 104 tests: six reactor, 71 scan, one startup
matrix, 17 vehicle-character, two vehicle-injury and seven vehicle-mass
(`target/audit-pilot-counts-acceptance.log`). The 42 unit-field and two character
clear tests also passed earlier in `target/audit-pilot-counts-regressions.log`;
that log additionally contains six superseded character-fixture failures.
Clippy with warnings denied passes in `target/audit-pilot-counts-current-clippy.log`.
Formatting/diff checks pass; reference files remain unchanged.

LIVE session 51739 runs crew, movement, driving, unit-field and character-clear
checks in `target/audit-pilot-counts-motion.log`. Character-clear (two) and crew
(five) are already green; movement is still executing, with no observed failure.
LIVE full-suite session 90424 continues in `target/audit-startup-health-full.log`.
It has recorded the simple reactor oracle failure and four contact wreck-fixture
failures, all corrected and passing focused checks above. It began before the
recovery feedback and shared-count changes, so it is not full acceptance of the
current tree. Do not restart either live process solely because of a timeout.
Sessions 50185 and 91961 are reaped with exit zero. No replacement full run started.

## Completed pilot-health baseline and UI inventory corrections (2026-09-14)

Reaped full-suite session 90424: exit 101, 346 completed suites, 2,616 passed and
six failed. The failures were one incorrect simple-reactor expected total,
four stale contact wreck fixtures, and command registration inventory missing
`+charclear`/`+show`. All six are corrected. Re-running the three affected suites
passes 94 tests (six reactor, 71 scan, 17 commands) in
`target/audit-baseline-corrections.log`. This does not turn the old full log green
or claim a new full-suite run for the entire current tree.

Reaped movement follow-up session 51739: exit zero, all 429 tests pass (two
character-clear, five crew, 358 movement, 42 unit-field, 22 vehicle-driving), log
`target/audit-pilot-counts-motion.log`. There are no running full-suite or movement
checks now; do not resume the old handles or start replacements on the assumption
that their observation timed out.

Closed the map-display bounds verification item against reference
`core/configuration.c::btech_player_ui_preferences_set`: tactical width 5–40,
height 5–24, LRS height 10–40; defaults 21/14/11. Added all eight min/max combinations
and the missing opposite-side rejection checks to the native persistence test.
The 71-test scan suite passes with those checks.

Corrected the audit's `@stat` inventory: `do_show_stat` is registered as `+rolls`.
The command is still missing. Added `docs/btech-roll-statistics.md` to record the
actual histogram, display arithmetic and generic-roll versus character-roll
boundary. A partially counted placeholder report would not satisfy this gap.
The active multi-subsystem goal remains open.
All-target Clippy with warnings denied passes in
`target/audit-baseline-corrections-clippy.log`; formatting and diff checks pass.
Reference tree unchanged. All processes started in this continuation are terminal.

### Retained map decoration duration (2026-09-14)

Added `BattleDecoration.object_duration`, a persisted signed-short value distinct
from the event countdown. A common constructor initializes all gameplay marker
creation paths. Operator smoke retains the clamped original input, including
negative durations whose event runs next tick. Fire decrements the retained
budget exactly once per spread, derives the next timer from it, and keeps the
budget unchanged during burnout. This supplies the duration prerequisite for the
reference `LIST OBJS` table; table formatting, remaining payload projection and
ordering are still open.

Updated the pre-1.0 `btech_map_decorations` schema directly; existing databases
with that owned table need the new `object_duration` column and correctly sourced
values. No fabricated backfill from elapsed countdowns or compatibility fallback
was added. A source duration cannot be recovered exactly after its countdown has
already advanced.

Verification: final decoration/surface suites pass 110 tests in
`target/audit-decoration-duration-fire.log`. Earlier focused terrain, export,
imported-decoration, artillery, inferno and listing suites also pass; all-target
Clippy, formatting and diff checks pass. The reference tree remains unchanged.

### Reference map-object table (2026-09-14)

`LIST OBJS` now renders the reference header, fixed-width numeric columns and
separator footer through one native/Lua publisher. Typed records supply the
object reference, byte, signed-short and scalar fields. Active smoke reports
its retained duration after its timer advances; imported decoration, building,
linked-marker and landing-block payloads remain visible after restart. Linked
markers now expose complete records through Lua map inspection; coordinates
are under `coordinate`. Coordinate-only marker edits preserve the payload.

Final listing/link/deletion/resize checks pass 13 tests in
`target/audit-object-table-final.log`. Imported decorations, wrapping and terrain
editing also passed their focused checks in `target/audit-object-table.log`.
All-target Clippy, formatting and diff checks pass. No reference files changed.
Ordering across authored records and active effects, and the reference's TBITS
information-object line, remain open; this is not full LIST parity acceptance.

### Active map-effect ordering (2026-09-14)

Active fire/smoke objects now retain an independent creation-order value. The
shared installation path prepends new and replacement effects within their kind;
listing combines that order with imported restoration ordinals. Tile coordinates
and timers no longer determine these rows' order. The same selection helper is
used for map-object deletion. Negative order values and uniqueness within each
kind are validated; exhausted ordering fails before publication.

The owned pre-1.0 decoration table now also requires `creation_order`. This is a
schema change, with no guessed backfill from tile numbers. Final focused suites
pass 25 tests in `target/audit-decoration-order-final.log`; the broader surface
run passed 105 tests before the final validation/test additions. All-target
Clippy, formatting and diff checks pass. Reference files remain unchanged.

Other object-kind ordering remains open. TBITS is allocated on set and unset
operations and can survive removal of the final mine or entrance, so current
mine/building presence alone cannot reproduce its diagnostic line.

## VTOL landing and movement reachability (2026-09-14)

Rotor-loss landing now reports `The rotor's dead!`. The material test verifies
the exact reply, unchanged refused state, and fuel refusal taking precedence
when the aircraft also has an empty tank. All 28 VTOL flight/control tests pass
in `target/audit-vtol-rotor-landing.log`; formatting and focused diff checks pass.

Read-only tracing corrected two audit assumptions. Reference
`mech_motion_integrate` stops landed MOVE_VTOL before hex transitions; the
landed taxi branch belongs to MOVE_FLY. No VTOL taxi motion was introduced.
The reference vertical altitude check uses the speed-based crash formula
already implemented in Rust. The drop-height formula belongs to horizontal
terrain collisions. Landed throttle admission and airborne forest/elevation
collisions remain open, as does the rest of the multi-subsystem audit.

## Grounded VTOL throttle (2026-09-14)

The shared vehicle throttle control now admits landed and launch-preparation
requests. This sets desired speed without adding ground travel. Liftoff retains
the existing reference behavior: horizontal actual and desired speeds reset to
zero, with a 60 KPH climb. The native/Lua regression checks both grounded phases,
callback output/state rollback, save/reload, stationary countdown ticks and
liftoff state. Flight help describes these controls.

All 51 flight, flight-control and ground-driving tests pass in
`target/audit-vtol-grounded-throttle.log`; focused Clippy with warnings denied,
formatting and diff checks pass. Four ground-driving smoke fixtures now supply
the required retained duration and creation order introduced by the earlier
map-decoration work. Grounded heading controls, horizontal throttle fuel
admission and airborne terrain collision handling remain open.

## VTOL heading and throttle fuel admission (2026-09-14)

Grounded and launch-preparation heading requests now use the shared vehicle
control path. Horizontal throttle requires fuel, including stop requests, and
reports `You're out of fuel!` on refusal. Native and Lua commands both supply
the host's fusion-fuel policy; the exemption applies only to fusion engines.
Heading requests and speed readouts remain available with an empty tank.

The reference `mech_move_event` returns for landed VTOLs before updating heading
or speed. Rust likewise retains the selected bearing through grounded and
countdown ticks. A new regression exposed that liftoff incorrectly cleared
desired heading: takeoff now uses the existing translation-only stop helper,
matching reference `mech_movement_stop`, which clears only actual/desired speed.

The final 52 flight, control and vehicle-driving tests pass in
`target/audit-vtol-control-admission.log`. The fuel matrix covers landed,
launching and airborne phases with ICE/fusion engines, empty/nonempty tanks,
both exemption settings, native/Lua agreement, unchanged refusals, readouts,
heading rollback and persistence. Focused Clippy with warnings denied and
formatting/diff checks pass. Airborne forest and elevation collision handling
and the broader multi-subsystem audit remain open. Reference files are unchanged.

## Airborne VTOL forest entry (2026-09-14)

The traced flight path now reports horizontal entry below light/heavy forest
canopy as a distinct unresolved contact. The world resolver requires an active
pilot and uses the shared +5 terrain control check. Success retains the prior
position and altitude and stops translation. Failure, including disconnected
crew, places the aircraft at entry and composes the shared one-level crash.
The existing crash service disables flight and publishes the reference rotor
notice; that notice does not independently erase rotor-section material.

The live movement publisher carries the forest-specific notices, observer
broadcasts, fall effects and experience messages without the unrelated vertical
ground-impact banner. Material-only contact remains unchanged until the world
supplies pilot context. Forest hover and vertical surface-contact rules remain
separate from horizontal entry.

Final focused flight/control/ground-driving suites pass 54 tests in
`target/audit-vtol-forest-final.log`. Tests cover both forest types, exact canopy
clearance, intermediate hexes, no horizontal entry while hovering, deterministic
+5 success/failure thresholds, disconnected pilots, one-level damage, unchanged
successful rollback material, reconnect-aware save/reload replay, post-crash
persistence and live output. All-target Clippy with warnings denied, formatting
and diff checks pass. The reference tree remains unchanged. Horizontal elevation
collision avoidance and its drop-height crash formula remain open, along with
the broader audit requirements.

## VTOL horizontal elevation collisions (2026-09-14)

Horizontal terrain entry now uses the shared jump-entry predicate with the
reference flight rule for intact ice. Forest and elevation contacts share one
host obstacle result and publication path. Elevation contact rolls back to the
previous hex at that terrain's elevation, then attempts emergency landing with
a piloting modifier of truncated elevation divided by three. Assigned pilots
also need grass, road or building terrain; an unassigned pilot succeeds without
a roll. Safe landing stops actual and vertical speed while retaining throttle.

Failure uses shared aircraft crash and vehicle fall material. The reference's
drop-height helper subtracts the selected surface twice after rollback; zero
and negative severity are preserved for the crew check, with zero structural
damage. Unsigned public fall entry points still validate their range before
delegating to the same signed material implementation. VTOL domino needs no
additional effect because the reference resolver rejects non-Mech callers.

Tracing `mech_hex_entry_resolve` and `mech_position_rollback` also corrected the
previous forest implementation: successful avoidance restores terrain elevation,
not the previous airborne altitude. The regression expectation now checks this
explicitly. Existing slow-hill and intermediate-before-map-edge tests now expect
an unresolved obstacle or the pilotless emergency landing instead of automatic
touchdown/crashing in the entered hill. A stale grounded-throttle rejection in
the crash suite was updated to the admitted control behavior.

All 72 flight, control, crash, ground-driving and fall-heading tests pass in
`target/audit-vtol-elevation-final.log`. Coverage includes bridge underside and
ice entry, water immersion, emergency landing, signed crew modifiers, positive
and zero damage cases, pilotless admission, saved-state replay and live messages.
All-target Clippy with warnings denied, formatting and diff checks pass. Help
and the main audit are updated; reference files remain unchanged. This closes
the audited horizontal elevation-collision path, not the broader subsystem goal.

## ECM status lamps and network target format (2026-09-14)

Enabled Guardian and Angel ECM now render red when the committed field says
countered, green otherwise. The renderer previously tested running power, so
countered running suites displayed the wrong color. The existing shared loop
handles Mech and vehicle equipment without another state owner or field refresh.
Tests cover all Off/ECM/ECCM countering combinations, read-only native/Lua output
and restart. The live vehicle electronics test now checks emission, countering
by enemy Angel ECCM and destroyed-suite `XX` through real state transitions.

The C3/C3i target format audit entries were stale: both families already share
the `c:` column and unidentified `something`/blank-status renderer. Existing
tests cover peer identification, native/Lua privacy and replay; explicit rendered
physical/network range and unknown-name/status assertions now supplement them.
The main audit records those named output items as implemented without claiming
other sensor or unsupported-unit work is complete.

All 43 status, vehicle-electronics and command-network tests pass in
`target/audit-status-ecm-countering-final.log`. Focused Clippy with warnings denied,
formatting and diff checks pass. No snapshots were regenerated and the reference
tree remains unchanged. The broader multi-subsystem goal remains active.

### Map command token boundaries and landing-block admission (2026-09-14)

Native LIST now uses the reference space/tab delimiters, preserving Unicode
whitespace inside invalid target tokens. ADDBLOCK consumes at most four fields,
ignores subsequent fields and uses the reference missing-field, numeric and
coordinate error replies. Numeric validation precedes coordinate admission;
signed zero, optional teams and full signed integer bounds remain supported.
The shared landing-block action supplies coordinate diagnostics to native and
Lua callers without changing exclusion geometry or duplicating chassis rules.

All nine map-listing and landing-block tests pass in
`target/audit-map-command-admission.log`. Coverage includes malformed numbers in
every field, overflow, Unicode whitespace, ignored suffixes, read-only rejection,
native/Lua agreement and the existing restart and callback rollback scenarios.
The obsolete expectation that a fifth ADDBLOCK field prevents insertion was
removed after checking the reference's bounded tokenizer.

Read-only TBITS investigation also established that allocation blocks resizing
before argument parsing, while persistence preserves only allocated rows.
Deleting the cache changes mine/hangar admission independently of its underlying
objects. Those requirements are now explicit in the main audit; no partial cache
implementation was added. The broader integration run remains in progress in
`target/audit-current-integration.log`, and the multi-subsystem goal remains open.

The ongoing integration run exposed two obsolete VTOL collision expectations.
The hiding test now uses a forest collision to verify movement reveals the unit
before crash damage; its stationary descent still verifies damage-driven cover
loss. The wrapped-map test now expects detection at the opposite edge followed
by pilotless elevation avoidance on the departure hex, retaining desired speed.
Both retain replay assertions, and the hiding case retains host rollback checks.
All 11 hiding tests and five wrapping tests pass in
`target/audit-hiding-obstacle-replay.log` and
`target/audit-wrapped-elevation-replay.log`. Formatting and diff checks pass.
Focused Clippy also passes with warnings denied in
`target/audit-map-admission-clippy.log`; the reference tree is unchanged.
These focused corrections do not turn the still-running baseline into a full
suite pass; its log contains the two original failures.

### Landing-block creation order and shared persistence (2026-09-14)

New landing blocks now prepend their kind's traversal, matching
`map_add_block`/`add_mapobj_to_type`. Stable record ordinals and payloads remain
independent of that traversal. Explicit slot updates retain position; removals,
map clearing and owner purges maintain exact membership. Map reload preserves
the order. Native LIST and DELOBJ use the same traversal.

Mine and landing-block order now share persistence code, with kind-selected
owned table names. Reference imports without owned order metadata retain ordinal
order. Saves preserve auxiliary object columns instead of renumbering records.
Order load validation rejects duplicate, absent and unknown slots; read-only
loading does not create tables. Maintenance recognizes both owned order tables
and removes their entries when a map is purged.

All 23 mine, landing-block, map-listing, deletion, load and resize tests pass in
`target/audit-landing-order.log`. The final 10-test run in
`target/audit-landing-order-final.log` includes imported extension-column
preservation, aborted native/Lua transactions, restart, corrupt-order rejection
and map purge, plus two hovercraft water tests. The latter had handwritten smoke
fixtures missing required retained-duration/order data; they now use the shared
decoration constructor. Focused Clippy passes with warnings denied in
`target/audit-landing-order-clippy.log`; formatting and diff checks pass.
The reference tree remains unchanged.

The integration run in `target/audit-current-integration.log` finished with
2,635 passing and five failing tests across 347 targets. Four were the fixture
failures now covered by focused checks: VTOL hiding, wrapped elevation avoidance
and both hovercraft water tests. The fifth exposed this change's initial purge
wiring: the TCP repair test used a rebuilt server binary, which attempted to
delete from absent optional order tables. Cleanup now uses the shared optional
table probe before deleting order rows, then performs ordinary dependency checks.
This run overlapped edits and is not evidence of a full-suite pass for the current
tree. Other map-kind ordering and the broader audit remain open.

The final focused run passes all 25 map/mine/hovercraft tests in
`target/audit-landing-order-complete.log`, and the TCP repair regression passes
in `target/audit-order-optional-purge.log`. Clippy with warnings denied passes
for those targets and foundation in `target/audit-landing-order-clippy-final.log`.
A fresh full-suite verification is being run in
`target/audit-post-order-integration.log`; it must finish before another full
integration claim can be made.

That fresh run has now completed successfully: **2,641 tests passed, zero failed,
zero ignored, across 347 targets**, including the documentation-test target.
`target/audit-post-order-integration.log` records the complete run. Source, test
and game files were held stable throughout compilation and execution; only audit
documentation changed. The five previously failing tests all pass, including
the TCP database-repair regression. This is the current integration baseline,
not a claim that the remaining functional audit requirements are implemented.

### TBITS startup and consumer inventory (2026-09-14)

While the fresh full suite compiles, source and test files are held stable.
Read-only reference tracing found that `load_update3` rebuilds mine bits after
special-state restoration. Thus saving no TBITS rows does not imply that a map
with surviving mines will have no cache after startup. Hangar bits are not
rebuilt by that pass. Their direct consumer gates structure-entry CF notices;
adding the gate to every building lookup would incorrectly change other actions.

`docs/btech-map-bits.md` records row allocation, set/unset/clear behavior,
deletion, resize precedence, persistence validation, startup reconciliation,
consumer placement and required acceptance cases. It also records the reference's
uninitialized coordinate members on newly allocated TBITS objects; no deterministic
coordinate-deletion behavior is claimed for that undefined case. This narrows
implementation uncertainty without introducing a partial cache. The implementation
gap and multi-subsystem goal remain open, and full-suite verification continues
in `target/audit-post-order-integration.log`.


## TBITS acceptance progress (2026-09-14)

Corrected five stale test expectations for owned lookup rows and valid TBITS
operations. The revised terrain test exposed a real reload defect: retained rows
lost their map-object flag when new terrain flags were installed. Reload now
retains that flag. Database rollback tests verify byte updates and preservation
of the hangar lane, padding and a second map. Step-notice tests now cover deleted
TBITS across all five mobile ground chassis without suppressing entrance lookup.

`target/audit-map-bits-revised-final.log`: 30 integration tests pass across six
targets. `target/audit-map-bits-revised.log`: two lookup unit tests pass (other
targets in that invocation were filtered out). Remaining acceptance is recorded
in `docs/btech-map-bits.md`; the broader six-subsystem goal remains open.


## TBITS consumer acceptance (2026-09-14)

Added explicit stale-hangar clearing checks to UPDATE LINKS, including retained
zero rows, native/Lua agreement, rollback and restart. Added coordinate-deletion
coverage proving that surviving mines are rebuilt after TBITS removal. All seven
supported chassis now verify mine-selection and scan suppression without dice
consumption after deletion, with detection restored by an explicit rebuild.
Twelve consumer tests pass in `target/audit-map-bits-consumers.log`.

The broader twelve-target run passed 164 tests with one stale artillery duplicate
expectation. Corrected that expectation to require uncached deposits to prepend
and covered deposits to suppress reinforcement; all four ADDMINE tests then pass
in `target/audit-map-bits-artillery-order.log`. The combined evidence covers 165
tests in those targets. A current full-suite check is the next verification gate;
this does not close the remaining combat, movement, sensor, character or UI gaps.


## Full TBITS regression result and corrections (2026-09-14)

`target/audit-map-bits-full.log` completed with 2,647 passed, two failed and none
ignored across 348 targets. Source and test files remained unchanged throughout
compilation and execution. The process was reaped with exit 101 before fixes.

The building-exit fixture enabled gravity by replacing all exterior map flags,
removing the existing lookup-object flag. It now preserves unrelated bits. All
15 tests in that target pass (`target/audit-building-exit-flags.log`).

The kick counter test used an unseeded target and counted only the final kick
balance fall. Critical-induced falls occur earlier and are reported separately.
It now checks 32 deterministic target seeds across five hit/glancing cases,
includes immediate impact falls in damage taken, and requires that such a fall
actually occurs. It retains replay equality, direct-only attacker credit and
shutdown/restart assertions. The test passes in
`target/audit-kick-counter-cascade.log`; formatting passes. No combat rule was
changed to satisfy the test. The full baseline itself was not rerun after these
test-only corrections.

TBITS implementation and its named consumer acceptance are verified by the
focused and broad runs above. The next open implementation is sprint; the live
contract and caller inventory are in `docs/btech-sprint.md`. The original
six-subsystem goal remains open, including other audit gaps.


## Sprint state and admission checkpoint (2026-09-14)

Added typed saved sprint mode to both anatomy stores and their status2 projection
and setter. A shared getter/setter supports reverse admission and orbital
insertion; towing refusal retains precedence. Charge selection rejects sprinting,
and successful orbital insertion clears it inside the existing transaction.

Library checking passes. Fifteen tests pass in
`target/audit-sprint-state-tests.log`, including all seven chassis for saved state,
Lua rollback and orbital insertion. Mobile chassis verify reverse refusal;
Mechs verify charge selection refusal. Formatting passes. Sprint speed effects
and the remaining acceptance matrix are not yet implemented; see
`docs/btech-sprint.md`. The broader audit remains active.


## Sprint shared speed arithmetic (2026-09-14)

Added one shared helper for reference-order additive boosts, myomer rounding,
pilot bonus and gravity. Effective-speed queries now read sprint and the current
pilot's exact-one Speed_Demon advantage for both anatomy stores. Jump load
checking also supplies the live pilot input. Two arithmetic unit tests pass;
100 integration tests pass across the corrected sprint target and affected
battle-value, building-entry, jump and vehicle-XP targets. Logs and exact scope
are in `docs/btech-sprint.md`; formatting passes.

Remaining sprint work includes configured TSM policy propagation, throttle,
actual movement, turning, validation envelopes and their acceptance matrix.
The default-enabled policy in the current effective Mech path is explicitly an
intermediate state. The six-subsystem objective remains open.


## Sprint host policy propagation (2026-09-14)

Introduced shared towing/sprint myomer policy and routed configured building
entry, delayed rechecks, exit and prone handling through the effective-speed
core. A host-level hot-sprint admission test distinguishes enabled and disabled
TSM policy and verifies rejected admission preserves state. The existing towing
policy test remains intact. All 24 focused tests pass in
`target/audit-sprint-host-policy.log`; library and formatting checks pass.
Throttle, actual motion, turning, validation and remaining speed consumers are
still open. See `docs/btech-sprint.md`; the full audit goal remains active.


## Sprint throttle and movement integration (2026-09-14)

Connected configured controls, Lua state and status to the shared sprint throttle.
Ground proposals, turning and VTOL updates now apply world-aware sprint bonuses;
Mechs retain their additional update/turning conversion. Server movement rules
carry the TSM sprint switch. Saved sprint limits and vehicle restoration admit
legal sprint controls without changing non-sprint validation.

All tests type-check. Sixty-four focused tests pass in
`target/audit-sprint-motion.log`. Five mobile ground chassis accelerate beyond
their ordinary ceiling, agree with the Lua readout and replay movement after
save/load. Formatting passes. Sprint-specific VTOL flight, mode/policy changes at
speed, combined boosts/load/gravity and remaining consumers are still open in
`docs/btech-sprint.md`. No claim of full sprint or six-subsystem completion.


## Sprint retained controls and airborne acceptance (2026-09-14)

Recovered completed verification: 52 focused integration tests and 239 library
tests passed after the retained-speed validation fix. Clearing sprint at speed
had rejected valid motion; persisted bounds now retain previously legal controls
independently of the current mode. This supersedes the preceding statement that
non-sprint validation was unchanged. Actual propulsion still gates immobile units.

Extended mode-clear replay coverage to airborne VTOLs and checked fresh controls
against the reduced ceiling for all six mobile chassis. Ground direct requests
above it fail without mutation, VTOL requests clamp, and native `speed run`
requests the current ceiling. The five sprint tests pass in
`target/audit-sprint-controls-final.log`; formatting passes. The separate airborne
acceleration case also retains vertical speed through saved replay. Remaining
sprint acceptance and policy consumers are listed in `docs/btech-sprint.md`;
the full audit is not complete.


## Sprint policy in combat experience and battle value (2026-09-14)

Fixed host policy propagation into classic gunnery XP, battle-value XP and the
`bv` inspection field. Shared shot contexts now carry the hot-TSM sprint switch
for Mech and vehicle firing, and both XP participants use the configured speed
calculation. Battle-value XP retains nominal speed in its separate speed factors.
Native and Lua field inspection share configured valuation without mutating state.

A new matrix exercises attacker/target policy effects across distinct classic
and BV speed bands, both host field settings, and saved XP replay after pilot
reconnection. Seventy-eight affected integration tests and 239 library tests
passed; see `docs/btech-sprint.md` for exact logs and intermediate fixture fixes.
All tests type-check and formatting passes. Prediction/landing and jump policy
consumers, combined movement cases and the other audit requirements remain open.


## Sprint policy in jump admission and predictive fire (2026-09-14)

Fixed the unconditional hot-TSM sprint bonus in jump cargo admission. Native/Lua
projected jumps and DFA now use the host setting through the same launch path.
Snipe, manual landing and server jump/landing movement rules also carry the
configured switch.

A cargo boundary test verifies opposite admission under the two settings,
read-only refusal, post-admission callback rollback, and saved native/Lua replay.
A predictive-fire test verifies distinct selected hexes under the two settings,
exact agreement with live movement over shell travel time, read-only forecasting
and matching native/Lua queued aim points after reload.

All 87 tests across jump, snipe, sprint and VTOL controls pass in
`target/audit-sprint-jump-prediction-regression.log`. All tests type-check and
formatting passes. Combined booster/load/gravity motion, policy changes at speed,
tail-rotor consequences and raw mode import remain open; full audit completion is
not claimed.


## Loaded sprint gravity and shared vehicle queries (2026-09-14)

Fixed the effective-speed vehicle adapter discarding map gravity. It now uses the
same world-aware bonus/environment helper as controls and movement. A tracked
vehicle at the gravity floor previously reported half its legal sprint throttle.
The new matrix covers six mobile chassis, zero/seven cockpit crates, ordinary
versus special maps, gravity 0/50/100/200, exact five-tick acceleration, validation,
read-only queries and saved replay.

The shared fix also changes vehicle building-exit reconciliation on special maps.
Updated its old vehicle-only test exemption to expect the explicit 2g speed factor,
retaining the other transfer and flight assertions. All 116 affected integration
tests pass across `target/audit-sprint-gravity-final.log` and
`target/audit-sprint-gravity-consumers-final.log`; formatting passes. The reference
tree remains unchanged.

Ordinary non-sprint throttle/proposal gravity consistency still needs review.
Simultaneous equipment boosts, hot-TSM policy changes at speed, tail-rotor
consequences and raw reference mode import remain open. The six-subsystem goal
is not complete.


## Ordinary movement shares map gravity (2026-09-14)

Fixed ordinary throttle, ground proposals, turning and VTOL movement omitting
map gravity. Added one world-aware environmental step shared with sprint,
preserving ordinary-map and 100% gravity precision while applying the reference
single-precision factor and 50% floor elsewhere. Removed an unused turning wrapper.

The cargo/gravity replay matrix now covers both movement modes across six mobile
chassis, including airborne VTOLs. It reproduces the original 118.25 versus 236.5
KPH disagreement, checks effective/throttle agreement and five exact acceleration
ticks, validates the world and compares saved replay. All 122 affected integration
tests and 239 library tests pass (`target/audit-ordinary-gravity-regression.log`,
`target/audit-ordinary-gravity-unit.log`); formatting passes. Reference files are
unchanged. This closes the ordinary-mode gravity gap identified in the preceding
checkpoint; simultaneous equipment boosts, hot-TSM policy changes, tail-rotor
consequences and raw mode import remain open within the wider audit.


## Combined boost movement and hot-TSM policy changes (2026-09-14)

Added live-motion acceptance for 48 combinations of MASC, supercharger, sprint,
cargo and gravity, using installed hardware and ordinary booster toggles. Checks
cover reference-order throttle/update arithmetic, five acceleration ticks, exact
requested controls, validation and saved replay. Ordinary f64 control values are
compared at explicitly bounded f32 precision rather than claiming bitwise parity.

Added enabled/disabled/enabled host TSM sprint policy changes while moving. Lua
readouts use each new setting, acceleration changes accordingly, existing controls
remain intact and each stage replays from a saved world. All 19 sprint/booster
tests pass in `target/audit-sprint-boosters-policy-regression.log`; formatting
passes. No production changes were required for these acceptance cases. Tail-rotor
consequences, raw mode import and the broader audit requirements remain open.


## Tail-rotor sprint damage and repeated hits (2026-09-14)

Fixed first tail-rotor damage limiting speed against the unloaded chassis ceiling.
A shared world transition now uses current load, sprint and gravity, reserves the
vertical velocity component, applies the reference cruise-minus-0.1 request and
preserves actual momentum. Repeat damage no longer reapplies the limit. First-hit
and repeated-hit messages now match the reference.

A live Kestrel test reproduces the old 128.856926 versus 336.044097 KPH mismatch,
checks the corrected request, fresh controls, saved replay and a repeated hit
after gravity changes. Ordinary hit tables do not directly select tail damage;
that initial suspected bypass was disproved, and the shared transition is used
for their reachable rotor material outcomes and selected criticals.

All 40 affected integration tests and 239 library tests pass in
`target/audit-sprint-tail-final.log` and `target/audit-sprint-tail-unit.log`.
Formatting passes, reference files are unchanged, and raw sprint-mode import plus
the broader audit remain open.


## Reference import dependency and roll-report foundation (2026-09-14)

Verified that raw sprint-bit import depends on full reference-unit reconstruction:
the loader reads Rust-owned complete records and does not read the reference
runtime/construction tables. Documented this dependency without adding a partial
status overlay or claiming import completion.

Implemented `BattleRollStatistics`, an explicit generic-roll histogram and the
reference-format diagnostic renderer. Tests cover exact empty/edge/middle rows,
all buckets, total counts, read-only rendering, independent candidates, rejected
inputs and atomic overflow. All 241 library tests pass in
`target/audit-roll-statistics-unit.log`; formatting passes. No reference files
were changed.

Live accounting and `+rolls` remain incomplete. Next work must classify generic
checks separately from direct character dice and integrate simulation-lifetime
counts at native, Lua and server commit boundaries, including deletion and
rollback behavior. The public histogram is a foundation, not an exposed command
with partial live totals. The full six-subsystem goal remains active.


## Explicit roll journals and first classified combat callers (2026-09-14)

Added a process-local generic-roll journal to `BattleDice`. Candidate clones own
independent journals, generator/gameplay equality remains about the random stream,
and saved data contains only the tagged generator. Direct serialization preserves
its u128 stream position; the initial serde-flatten approach failed restoration
and was replaced. Taking a journal does not consume random words.

Ordinary attacks and caseless propellant checks now record explicitly. Dead-fire
and below-minimum extended-LRM attacks retain direct three-die behavior and are
not counted. Tests exercise that distinction, journal isolation, unchanged random
outcomes and subsequent draws, drain behavior and fresh diagnostics after reload.
All 243 library tests and 51 related integration tests pass in
`target/audit-roll-journal-unit-complete.log` and
`target/audit-roll-journal-combat.log`; formatting passes. Reference files remain
unchanged.

The global history owner, retirement/deletion handling and remaining generic-roll
callers are still required. `+rolls` is not exposed with these partial counts.
The broader audit remains active; see `docs/btech-roll-statistics.md` for the
classified call sites and ownership work remaining.

## Environmental and control roll accounting (2026-09-14)

Classified generic checks now include Mech/vehicle piloting, orbital landing,
boosters, ordinary heat checks, woodland effects, fire spread, analog radio
distortion, iNarc pod swatting and exposed searchlight damage. The separate
Computer skill roll remains excluded, as do duration, replacement-terrain and
percentile dice. Reference paths are recorded in `btech-roll-statistics.md`.

The woodland matrix checks exact buckets and random-stream continuity across
2,304 combinations, including bare terrain and conditional third rolls.
Piloting checks explicitly verify no journal entries for automatic support or
blocked controls, followed by one entry for an admitted roll.

Verification: 244 library tests pass in
`target/audit-roll-environment-unit-final.log`; 29 environmental/control
integration tests pass in `target/audit-roll-environment-integration.log`.
After the pod and lamp changes, eight searchlight tests and five pod tests pass
in `target/audit-roll-searchlight.log` and `target/audit-roll-pods.log`.
Formatting and scoped diff checks pass; the reference tree remains unchanged.
This is focused verification, not a new full-suite baseline.

Simulation-wide history, owner deletion/replacement and remaining generic
callers are still open. The `+rolls` command remains unexposed until accounting
is complete; the six-subsystem audit goal remains active.

## Combat and indirect generic-roll callers (2026-09-14)

Classified the remaining production `two_d6` calls against reference behavior:
hit tables, material entry and criticals, physical attacks, charge/DFA, clusters,
swarm continuation, sixth sense, reactor/vacuum checks and vehicle fire/cocoon
damage. Ordinary two-die checks now record through the shared journal. Direct
hotload, punch/kick, critical-selection and effect dice remain excluded.

Also replaced hand-summed generic checks in Clan AMS, perception, RAC unjamming
and immobile aiming. IS AMS and targeting-computer selection retain single dice.
The aimed-location helper now uses the shared unit stream directly instead of
duplicating storage selection. Computer heat override remains the intentional
uncounted production `two_d6` caller.

A 768-case cluster matrix verifies ordinary versus hotloaded and energy behavior,
exact journal buckets and unchanged generator state. All 245 library tests pass
in `target/audit-roll-combat-unit.log`; 81 targeted combat, 358 movement/combat
and 91 auxiliary integration tests pass in `target/audit-roll-combat-integration.log`,
`target/audit-roll-combat-motion.log` and `target/audit-roll-combat-auxiliary.log`.
Formatting passes and the reference tree remains unchanged. No full-suite claim
is made from these focused runs.

Global history ownership and lifecycle acceptance remain open. The live purge
entry is `dbck.rs` calling `BtechState::purge`; ordinary unit creation rejects
occupied identities, and map replacement currently retains its old fire stream.
These are starting points for the ownership audit, not proof that all replacement
and deletion paths are covered. `+rolls` remains unexposed and the broader goal
remains active.

## Live computer failures and sensor recovery (2026-09-14)

Computer failure selection now runs from the server at the shared turn boundary.
Mechs, mobile ground vehicles and VTOLs use one host action for target loss,
shutdown and display outages. Shutdown retains ordinary fall, collision and crew
consequences. Stationary chassis are excluded, while running mobile vehicles
keep the heartbeat active even when stopped. Recovery events persist in an
ordered native queue, continue with parts disabled or power off, and preserve
overlapping outages and reference long-range/scanner restoration to 127.
Object purge and wreck retirement remove their queued events.

The live suite covers actual selected failures, all six mobile chassis fixtures,
stationary exclusion, stopped recovery, overlapping events, database restart,
later-unit rollback and real server commit failure/retry. All 250 library tests
and 20 computer/periodic-piloting/wreck-cleanup integration tests pass in
`target/audit-computer-runtime-lib.log` and
`target/audit-computer-runtime-final.log`. Formatting and diff checks pass;
the reference tree is unchanged. This is focused verification, not an updated
full-project baseline.

Remaining acceptance includes scanner/contact output at outage boundaries,
airborne shutdown casualties, destroyed recovery recipients and explicit object
deletion. Radio failures remain separate unfinished work. See
`docs/btech-computer-failures.md` for the behavioral contract and remaining scope.

## Computer display and lifecycle acceptance (2026-09-14)

Live per-display tests now cover tactical/LRS admission and native/Lua scanner
outage replies, preserved contacts and targets, recovery boundaries, destroyed
recipient silence and object-purge persistence. Forced shutdown is checked
against ordinary shutdown's material, crew, object and output consequences for
moving Mechs and vehicles and an airborne VTOL through its subsequent impact.
The server now checks computer failures after stagger handling, preserving the
reference's relative ordering. A broader scan regression had an outdated
bootlegger assertion that omitted nested fall-protection feedback; it now checks
that feedback and retains private-recipient assertions.

The source inventory also confirms that the radio branch of generic failure
selection has no active reference caller. Its private handlers and recovery
callback remain unused inventory; no new runtime trigger was invented.

All 250 library tests and 93 computer/scan/vehicle-scan/periodic-piloting tests
pass in `target/audit-computer-acceptance-lib.log` and
`target/audit-computer-acceptance-final.log`. Formatting and diff checks pass;
the reference tree is unchanged. This does not establish completion of the
broader combat, movement, sensors, map, character or UI audit.

## Maneuver piloting diagnostics (2026-09-14)

Standing, fast controlled drops and bootlegger pivots now publish their initial
piloting subtotal to MechDebugInfo at the captured feedback boundary. Standing
uses the non-awarding label; the two awarding checks use the reference `(noxp)`
label. Pre-roll maneuver warnings retain their position, automatic checks remain
silent, and cockpit feedback retains its private recipient. Both diagnostic and
ordinary reports share one notification-order implementation.

The new maneuver matrix verifies exact subtotal text, subscriber-only delivery,
warning/diagnostic/roll order, automatic quad standing and slow drops, native/Lua
agreement, saved state and channel counts, callback rollback and channel-counter
overflow rollback. A pre-existing prone assertion omitted nested fall-protection
messages; it now verifies those messages while retaining the private-audience
check. The implementation does not yet add diagnostics to nested fall checks or
all other control-roll callers.

All 250 library tests and 79 maneuver/prone/scan tests pass in
`target/audit-maneuver-diagnostics-lib.log` and
`target/audit-maneuver-diagnostics-final.log`. The expanded native-command matrix
also passes in `target/audit-maneuver-diagnostics-native.log`. Formatting and diff
checks pass; the reference tree is unchanged. The broader audit remains active.

## Stagger piloting diagnostics (2026-09-14)

Traditional, retained-history and consumed-history stagger checks now publish
the reference initial piloting diagnostic. Reports capture its notice boundary
before roll feedback, so severity/observer warnings remain ahead of it even
without an assigned pilot. The shared ordered publisher handles both maneuver
and stagger diagnostics. Blinded checks consume no diagnostic output, and failed
channel publication restores stagger history, rolls, damage and staged messages.

The new biped/quad matrix covers all three policies, assigned/private and
unassigned/cockpit recipients, blindness, exact diagnostic text and order,
database restart and channel-counter overflow rollback. Existing maneuver and
heartbeat suites pass, as do the ten stagger-filtered movement tests, including
character casualties and server save retry. All 250 library tests and 20 focused
integration tests pass in `target/audit-stagger-diagnostics-lib.log`,
`target/audit-stagger-diagnostics.log` and
`target/audit-stagger-diagnostics-motion.log`. Formatting and diff checks pass;
the reference tree is unchanged. Nested fall diagnostics and other audit gaps
remain open.

## Map membership span and operator output (2026-09-14)

FIXMAP now prints `Checking N entries..` using the allocated membership span.
LIST MECHS prints the reference first-free-slot diagnostic when the live unit
count differs. A map-owned scalar persists through the existing `first_free`
column; unit records remain the sole membership list. Native placement, transfer,
scenario changes, removal, object purge and wreck retirement update the scalar
through shared helpers. Removal shortens a final slot only once, preserving
interior and trailing holes until subsequent slot reuse/removal.

Tests cover sparse mixed-chassis listing, middle/tail removal, an empty but
allocated span, lowest-slot reuse, return to zero, read-only operator queries,
SQLite restart, placement, scenario commands, transfers and wreck retirement.
The existing wreck test now checks the changed allocation metadata separately
while still requiring all other map fields to remain unchanged.

All 250 library tests and 24 focused integration tests pass in
`target/audit-map-membership-span-final.log`. Formatting and diff checks pass;
the reference tree is unchanged. Invalid-reference repair branches remain
separate from the typed projection's validation, and unsupported DropShip map
messages remain open. The broader audit is not complete.

## Full regression run and registry contract (2026-09-14)

Started a full `cargo test --no-fail-fast` run after the computer-failure,
piloting-diagnostic and membership-span changes. Output is captured in
`target/audit-current-full-suite.log`; the run is still compiling at this entry,
so no completion or pass count is asserted here. Source code remains unchanged
during this baseline run.

Read-only examination of special-object registration, dispatch and help is
recorded in `docs/btech-special-object-dispatch.md`. It identifies actor/location/
contents ordering, handled access denials, signed class masks, restricted help
entries, categorized HELP ALL rejection and registration teardown requirements.
The audit links that contract; the registry gap remains unimplemented.

## Full-suite map and scheduler regressions (2026-09-14)

The ongoing baseline exposed a bulk-clear allocation bug: individual removals
can retain a trailing hole, but reference `map_shutdown_units` releases the
entire membership allocation and resets `first_free` to zero. Rust bulk clearing
now resets the map-owned extent after all removals within the existing rollback
checkpoint. The clear-map matrix checks zero extent and unchanged remaining map
fields across chassis, power states, native/Lua commands and restart.

Two stale regression expectations were corrected: vehicles now apply the shared
gravity multiplier, and running vehicle computers keep simulation pending when
parts are enabled. The digging test checks that computer-driven work remains
pending, then disables parts to verify digging alone keeps the scheduler alive
until completion.

All nine map-clear/environment tests and both runtime-statistics tests pass in
`target/audit-map-regression-fixes.log` and
`target/audit-runtime-stats-regression.log`. The original full run is still live
and uses its pre-fix executables; its failures are not a post-fix result. Other
audit gaps remain open.

## Empty allocated map acceptance (2026-09-14)

Extended bulk-clear acceptance to an empty map with a retained seven-slot
allocation. This verifies that clearing releases allocation even when the unit
iteration is empty, a Lua callback failure restores it, and the cleared state
survives SQLite restart. All four map-clear tests pass in
`target/audit-empty-map-clear.log`; formatting and diff checks pass. The original
full-suite process remains live, with the three previously corrected failures
still recorded against its pre-fix executables.

## Full baseline completed; catalogue queries (2026-09-14)

The original full run is now terminal: 2,696 tests passed and eight failed across
353 reported targets in `target/audit-current-full-suite.log`. All eight failing
tests subsequently passed targeted reruns. Bulk map clearing required a real
allocation reset. Other failures were stale expectations for vehicle gravity,
computer-driven scheduler work, retained environmental speed limits, nested
VTOL injury-avoidance feedback and the help corpus size. Help acceptance now
compares the index count to the actual article files while retaining per-keyword
reachability and rendering assertions.

Vehicle motion/motive suites pass five tests in
`target/audit-vehicle-regression-fixes.log`; that log also records an intermediate
VTOL failure. The final VTOL suite passes all eleven in
`target/audit-vtol-feedback-regression.log`, including disconnected obstacle
pilots whose crash still performs a separate injury-avoidance check. Help passes
all seven in `target/audit-help-corpus-regression.log`. Earlier map/environment/
runtime reruns are recorded above. This is a completed baseline plus targeted
fix verification, not a second full green run.

`special_commands.rs` now loads the prepared catalogue through typed special
object and class definitions. Lookup retains restricted matches, excludes
category separators and shares class filtering with help visibility. Tests
cover all five catalogues, unique executable names, space handling, known public
operator entries and the full signed-mask/class matrix. All 252 library tests
pass in `target/audit-special-catalogue-lib.log`. Formatting and diff checks pass;
the reference tree is unchanged. Runtime registry/help integration and the other
audit gaps remain open.

## Catalogue-driven help rendering (2026-09-14)

The shared typed catalogue now supplies `BattleSpecialType::help`: ordered
categories, class/privilege filtering, four-column command lists, named detail,
ALL restrictions, exact category errors, syntax colors and indented wrapping.
Menu cells and rules are shared with weapon specifications, preserving the
existing report layout. The help renderer remains independent of world mutation;
live special-object selection and HELP dispatch are not integrated yet.

All 254 library tests pass in `target/audit-special-help-lib-final.log`, and all
five weapon-report integration tests pass in
`target/audit-special-help-weapons.log`. Initial help assertions were corrected
to compare canonical styled text rather than noncanonical source color spelling.
Formatting and diff checks pass; the reference tree is unchanged. Remaining
registry admission/lifecycle and broader audit requirements stay open.

## Live special-object help (2026-09-14)

Uppercase HELP now enters BattleTech object selection before exit and general
native-command lookup. It checks the actor, location and persisted inventory
order, skips Zombie candidates, and renders the existing typed catalogue using
saved class codes and actor privilege. Lowercase/mixed-case help remains general
MUX help. Whitespace compression follows the host setting; uncompressed input
retains the reference's literal space delimiter and trailing topic spaces.
Unrelated command words bypass normalization and lookup immediately.

The existing containment-order calculation is shared with maintenance; no second
registry or inventory collection was introduced. An isolated C probe executes
the original dispatcher functions with stubbed registry/output services and
passes 16 casing/order/Zombie cases (`target/reference-help-probe.log`). This is
not an end-to-end reference server run. The reference tree remains unchanged.

All 254 library tests pass in `target/audit-live-special-help-lib.log`. The final
native help and maintenance run passes nine tests in
`target/audit-live-special-help-final.log`; the general command/help run passes
24 in `target/audit-special-help-dispatch-regression.log`. Acceptance includes
cockpit classes, imported DEBUG/AUTOPILOT/TURRET registrations, reversed saved
contents order, general-help fallback, read-only state and restart. Formatting
and diff checks pass. Ordinary command routing/restrictions and registration
lifecycle remain open, alongside the other audit requirements.

## Special-command restricted admission (2026-09-14)

The shared actor/location/inventory pass now checks ordinary command metadata as
well as HELP. Restricted first matches consume input with the reference denial
before exits or general native handlers can run. GOD/Wizard authority belongs to
the executor, including queued commands; a privileged cause is not sufficient.
Public first matches stop the search so later restricted candidates cannot
override them. Inventory projection is lazy when an earlier object matches.

All 62 distinct integration tests pass: two admission tests, three special-help
tests, 17 general-command tests, and 40 operator/map/scenario/snipe regressions.
Logs are `target/audit-special-admission.log`,
`target/audit-special-admission-final.log` and
`target/audit-special-admission-operators.log`. The admission tests cover exact
denials, prefix-independent restrictions, a colliding exit, persisted state,
candidate priority, Zombie exclusion and queued cause authority. Formatting and
diff checks pass; the reference tree is unchanged. Allowed commands still need
per-object handler routing/admission adapters, and registration lifecycle plus
the wider audit remain open.

## Selected MAP command execution (2026-09-14)

Allowed MAP matches now invoke existing handlers with the selected registration.
Nineteen map adapters share one target resolver. The actor remains in place;
world borrowing ends before a handler can mutate state. All 26 catalogue names
have bindings. VIEW uses the map handler, and STORES uses the actor-location
manifest; stock correction commands also retain the reference actor-location
exception. Global aliases do not rewrite a selected catalogue command.

Tests compare 21 map operations via inventory selection and direct map-location
selection, plus queued map-actor priority, an identically named exit, unchanged
other-map state, stock exceptions and restart. All 255 library and 105 integration
tests pass in `target/audit-map-dispatch-regression.log` and
`target/audit-map-dispatch-selected-final.log`. An initial test compile failure
used an unavailable inventory accessor; the test now uses the exported inventory
query. Formatting and diff checks pass; the reference tree remains unchanged.
Other type adapters, global fallback cleanup, registration lifecycle and the
remaining audit requirements stay open.

## DEBUG object command routing (2026-09-14)

Revalidated the existing status work and resumed the open special-object adapter
work. DEBUG now selects all nine catalogue commands before global dispatch.
SETWBV keeps its reference public access through the shared typed settings
mutation, while Lua/general operator APIs retain Wizard authorization. The
weapon adapter shares exact canonical part lookup and preserves reference
argument, bound and non-weapon replies. SETVRT retains audited operator mutation.
DEBUG SHUTDOWN requires a map argument, ignores missing maps and reuses atomic
map clearing; it cannot accidentally perform a no-argument cockpit shutdown.

Final verification: 272 tests passed (256 library and 16 integration) across
DEBUG dispatch, map clearing, selected-map routing, special admission and weapon
settings in `target/audit-debug-dispatch-final.log`. Formatting and diff checks
passed; the reference tree remains clean. The new test includes an imported
carried DEBUG object, public/restricted controls, an exit-name collision,
manufacturer identity, error precedence, restart reset, preserved registration,
separate typed/Lua authority and explicit-map shutdown behavior. This is progress
on the full audit goal. Other object adapters, registration lifecycle and full
DEBUG service-output parity remain open alongside the other audit sections.

## XPTOP menu and displaced ties (2026-09-14)

The previous goal turn made progress by adding selected DEBUG routing. Continued
read-only reference inspection found two real leaderboard mismatches: loose
text replaced CM_TWO cells, and stable sorting changed ties displaced by a later
leader. The report now uses shared menu cells (39 columns), blue rules (78), and
a one-cell total footer. Names are escaped before truncation. Native publication
uses the styled menu; the existing Lua report's text remains its plain projection.
Ranking preserves immediate strictly-greater exchanges, including displaced ties.
The bounded 10,000-player test still passes with this ordering.

Verification passed 261 tests (256 library, four XPTOP, one DEBUG dispatch) in
`target/audit-xptop-layout-final.log`. The new exact-output test verifies all
rows for balances 5/5/6, color separators, long literal markup-like names and
unchanged state. Existing tests verify native/Lua agreement, restart, limits,
finite zero-total handling and failed-publication rollback. Formatting and diff
checks pass; the reference tree is unchanged. This does not close remaining
DEBUG argument diagnostics, numerical differences, interactive menus or the
broader audit requirements.

## DEBUG character diagnostics and shared names (2026-09-14)

The preceding turn made progress on XPTOP formatting and ordering. This turn
continued the remaining DEBUG character controls: a single character-name helper
now supplies canonical and abbreviated names across values, advantages,
attributes and skills, reusing the existing catalogues. The skill catalogue's
abbreviation method delegates to the same helper. General value names moved out
of the show renderer without duplicating the catalogue.

SETXPLEVEL and XPTOP now emit reference-specific argument diagnostics, including
the distinction between unknown values and known non-skills. Error precedence
matches reference admission; successful threshold edits retain silent output,
shared operator authorization, logging and rollback. Exact-message integration
checks rejected inputs leave state unchanged and accepts a zero threshold by
skill alias. Library checks exercise each value family and malformed aliases.

Final verification passed 265 tests (257 library and eight integration) in
`target/audit-debug-character-final.log`, covering DEBUG, XPTOP and operator
transactions. Formatting/diff checks passed, and the reference tree is unchanged.
The broader goal remains active; these changes do not complete other special
object adapters, registration lifecycle or the remaining audit categories.

## DEBUG registration and teardown (2026-09-14)

The preceding turn made progress on character diagnostics. This turn began the
open registration lifecycle with DEBUG, which owns only a registration record.
The native @btech command now accepts /info, /register and /unregister, with
reference one-character abbreviations and Wizard switch metadata. Controlled
live Things can be registered as DEBUG without direct SQL. Same-type registration
and absent-registration teardown are idempotent; conflicting types are rejected.
Inspection accepts matched names and explicit identities. Containment is retained.

Persistence validation and writes now admit DEBUG registration insertion/removal
inside the existing world transaction. Other types retain domain-specific guards,
and purges remain owned by their existing cleanup path. Their initialization and
teardown are still explicit unfinished work, not silently accepted registrations.
In-game help documents DEBUG setup, its public SETWBV command and current limits.

Final verification passes 283 tests (257 library, 26 integration) in
`target/audit-debug-registration-final.log`. This includes complete native
register/use/save/reload/unregister/save/reload, exact diagnostics, idempotence,
protected-object authority, Going rejection, switch discovery, selected-map
routing, DEBUG commands and special HELP. The first integration run found the
expected switch inventory needed updating; its final fixture includes the
reference INFO/REGISTER/UNREGISTER metadata and passes. Formatting and diff checks
pass; the reference tree remains unchanged. The full audit goal remains active.

## MAP registration defaults and usable terrain (2026-09-14)

The preceding turn made progress on DEBUG lifecycle. MAP initialization now
extends that same registration command and delegates to the existing map
constructor. Read-only reference allocation inspection established 21x11 grassland,
Default Map, raw gravity/temperature/flags zero, daylight, visibility 30/max 60,
cloud base 200 and building regeneration one. These values are supplied directly
without adding another map representation or terrain parser. Same-type
registration remains idempotent and preserves live state.

The integration scenario registers a carried Thing, checks all 231 tiles and
initial settings, runs VIEW, repeats registration without state changes, saves
and reloads, then loads a 3x2 water map through native selected LOADMAP and saves
and reloads again. Existing map load, view and selected-dispatch suites also pass.
Final verification: 269 tests (257 library and twelve integration), zero failures,
in `target/audit-map-registration-final.log`. Formatting/diff checks pass and the
reference tree remains unchanged. In-game help includes setup and viewing steps.
MAP teardown and other types' lifecycle remain open within the full audit goal.

## MAP retirement and retained destinations (2026-09-14)

The prior turn made progress on MAP initialization. This turn implements MAP
unregistration through shared unit shutdown and membership removal, followed by
map identity retirement. Cleanup keeps the underlying world object, inventory
and containment. Reference lifecycle notifications target GOD; command admission
continues to use the actual executor. Going maps are accepted for teardown.
The map lifecycle transaction restores world and staged output on failure.

Persistence selectively deletes map-owned terrain dictionaries, raw tiles,
events, map objects, lookup rows, ordering extensions, slots and configuration.
It runs inside the existing world-save transaction, without treating the
container as a deleted object. Authored links to the retired map are cleared.
External entrance/exit markers retain object identities across reload and
reactivation, matching reference role removal. World validation still requires
extant non-garbage destination objects. Loaders no longer require those targets
to retain MAP roles. The restart scenario exposed a remaining repair-loader
assumption, which now skips inactive interiors. Building damage also treats an
inactive interior as absent rather than indexing a missing map.

Final verification passes 294 tests (257 library and 37 integration) in
`target/audit-map-teardown-final.log`, covering seven supported chassis,
registration, map clearing, building routes, entry, recovery and linking. Five
new scenarios cover durable teardown with inventory/containment retention,
external markers and Going-map reactivation, output-limit rollback, forced
SQLite-delete rollback, and an actual hit at an inactive building destination.
All sessions have completed. Formatting and diff checks pass; the reference tree
is unchanged. In-game help and the audit describe current behavior. Unsupported
unit families and the other special types' lifecycle remain open; the overall
audit goal is not complete.

## TURRET lifecycle and single-save role replacement (2026-09-14)

The previous goal turn made progress on MAP teardown. TURRET registration now
uses shared typed station defaults matching reference allocation. Its teardown
removes only owned station records and events, leaving game containment and
other stations unchanged. Inspection found the reference requires four turret
TIC rows; these are now loaded, validated and written as explicit station state.
The existing field-persistence test now updates an initialized TIC row, retaining
its check that gunner assignment preserves unrelated TIC values.

Role-change validation and save ordering now retire DEBUG/MAP/TURRET ownership
before admitting a replacement. Explicit unregister/register can therefore be
saved once without a registration collision. The new matrix covers all nine
source/destination pairs and verifies nonzero station TIC reset on recreation.
Station tests also cover reference sentinels, idempotence, Going teardown,
physical occupants, another station's independent clocks, restart and rollback
of timer/TIC deletion when SQLite rejects the station deletion.

Final regression passed 281 tests (257 library and 24 integration) in
`target/audit-turret-lifecycle-final.log`; all three lifecycle tests passed again
in `target/audit-turret-lifecycle-selected.log` after final formatting. Checks
cover gunner fields, targeting, reports, TIC actions, registration and MAP
teardown. Formatting/diff checks pass and the reference tree is unchanged. Help
and audit documentation describe the usable commands and remaining scope.
TURRET routing, MECH/AUTOPILOT lifecycle and opaque same-type map replacement
remain open along with the broader audit goal.


## Selected turret initialization checkpoint (2026-09-14)

The previous status-only turn verified existing work without closing another
audit requirement. This turn resumes concrete implementation: INITIALIZE and
DEINITIALIZE now retain the selected station before exit lookup. Shared station
admission accepts actor/location/carried candidates without relocating objects
or replacing the parent pilot. Takeover compares the previous connected gunner's
location with the actor's location, and repeated initialization uses the exact
reference joystick message. This builds on the already-tested selected turret
field adapters and deliberately inert TIC commands.

The new lifecycle scenario exercises all seven supported chassis, carried
stations, conflicting exits, connected-gunner refusal and takeover, repeated
initialization, deinitialization, parent-state/containment retention and restart.
Regression passed 271 tests (257 library plus 14 integration) in
`target/audit-turret-selected-lifecycle.log`. Formatting and diff checks pass;
the reference tree is unchanged. The earlier selected field/TIC regression also
passed 268 tests in `target/audit-turret-dispatch-final.log`.

Paused at the user's request after completing turret initialization. Sixteen
other turret commands still need selected-object adapters; MECH/AUTOPILOT
lifecycle, opaque same-type MAP replacement and the broader audit requirements
remain open. The overall goal is not complete.
