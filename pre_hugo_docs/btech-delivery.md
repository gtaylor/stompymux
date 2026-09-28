# BattleTech delivery audit

Historical audit through 2026-09-13. The current milestone boundary, validation
and prioritized backlog are in [the first implementation summary](btech-first-implementation.md).
This file retains detailed evidence; its earlier open items can be superseded by
later entries. Full reference parity remains follow-up work.

## Scope

Implement native Rust behavior for bipeds, quads, tracked/wheeled/hover/stationary
vehicles and VTOLs using the existing game directory. Share mechanics between
chassis; keep anatomy-specific state access at the boundaries. Do not modify the
`btmux-khi` tree. No runtime C bridge is present in the BattleTech domain.

Autopilot, repair systems, naval units, aerospace/dropships, infantry and battle
armor are deferred. Loading/unloading units into other units and container cargo
are also deferred; external towing remains included.

## Current evidence

- The latest construction audit accepts 1,299 of 1,302 biped/quad templates. The
  remaining PHX-HK2, STG-A5 and WSP-105 contain LAM conversion equipment. Do not
  silently discard that equipment to make the assets pass.
- The refreshed vehicle audit accepts all 269 supported ground/VTOL templates.
  This is construction evidence only.
- The uninterrupted full run in `target/listforms-full.log` passed all 2,196
  tests across 267 result targets, including documentation targets, and exited
  successfully. Code, compiled test executables and runtime test inputs were
  unchanged throughout the run. This covers LISTFORMS, SAVEDB, XPTOP, SETXPLEVEL,
  SETVRT/SETWBV and selected-map SHUTDOWN along with the earlier map, combat and
  lifecycle work. It replaces the 2,186-test baseline in
  `target/map-operators-full-verified.log`. Passing the suite does not establish
  completion of the remaining operator and scenario requirements.
- The reference cockpit catalogue contains 172 distinct command names. A literal
  comparison against Rust registration is only a candidate finder: aliases,
  combined commands and deferred unit types require individual review.

## Cockpit feature audit

| Feature | Reference contract | Current evidence | Required delivery |
| --- | --- | --- | --- |
| Weapon reports | `WEAPONSTATUS` diagnoses component damage; `WEAPONSPECS` lists distinct installed types and range statistics. | Native/Lua shared reports, seven-chassis access/state/restart tests and split-destruction checks pass. | Delivered catalogue-based reports; retain runtime weapon-setting override review. `CRITSTATUS` is delivered below. |
| Anatomical targeting | `TARGET` saves target anatomy separately from locks; direct aim and targeting computers affect accuracy and location selection. | Native/Lua controls, shutdown/restart, rollback, seven-chassis head aim, ordinary and computer-directed hits, absent VTOL turrets and rear damage dice are covered by shared tests. | Shared implementation is present. Retain AMS/Swarm ordering and changing-terrain or crash-during-salvo acceptance scenarios below. |
| Critical-slot inspection | `CRITSTATUS` lists all slots in a selected section, requires a conscious assigned pilot and permits mapless shutdown inspection. | Shared native/Lua inventory, all supported sections, anatomy, damage, split proxies, configuration, rollback and restart checks pass. | Delivered interface; retain broader operator/configuration and scenario acceptance review. |
| Sighting | `SIGHT` uses shared weapon/target preparation while bypassing expenditure and preserving cover. | Native/Lua unit, coordinate, observer and artillery sighting is implemented. Seven-chassis state/restart tests, special rolls, C3 feedback and rejection checks pass. | Delivered interface; retain broader configuration and aimed-section acceptance review. |
| TIC weapon groups | `ADDTIC`, `DELTIC`, `CLEARTIC`, `LISTTIC`, `FIRETIC` in `commands/mech_command_catalog.c`; behavior in `unit/mech_tic.c` | Membership, native/Lua ordered firing, shared readiness display and persistence are implemented. Shared native/Lua unit and coordinate targets and ordered mechanical admission are implemented. Outer firing gates and numeric edge cases remain under review. `weapon_groups.rs` groups damage packets, not pilot-selected weapons. | Shared group selection and ordered firing, native/Lua interfaces, saved group state, authority checks, restart and transaction tests for Mechs and vehicles. |
| Heat cutoff | `HEATCUTOFF` in `unit/mech_tic.c`: configuration gate, four-second transition, cutoff behavior in `movement/mech_update_heat.c` | Native/Lua toggle, durable countdown and disabled cooling state, thermal regulation and idle heartbeat integration are implemented. Lifecycle/environment/server tests pass, including idle completion with new commands disabled. | Idle toggle progression is separated from thermal sampling; focused lifecycle and server tests verify unchanged cooling and thermal clocks until accounting resumes. |
| Preferred ammunition location | `USEBIN <weapon> <location>` in `unit/mech_advanced.c` | Native/Lua control, saved typed preferences, shared feed ordering and cockpit inspection are implemented. Focused cross-chassis expenditure, fallback and mode tests pass. | Delivered with shared preference ordering, saved typed state, native/Lua controls, expenditure and mode/fallback tests. Retain automatic-defense lookup independence. |
| Automatic turret tracking | `AUTOTURRET` in `unit/mech_advanced.c`; pilot-controlled turret mode | Native/Lua mode, persistence and heartbeat tracking are implemented. Temporary sensor-flash blindness now suppresses tracking and recovers on the shared heartbeat. This is separate from the deferred autopilot. | Characterize tracking cadence and lock loss; implement durable mode and shared targeting integration, with jam/lock/destruction and restart tests. Tracking and sensor-flash suppression are verified. |

These are verified gaps, not an exhaustive list. Continue with the shared firing target argument audit and the remaining
cockpit/operator acceptance review. TIC firing reuses ordinary attack actions;
its rejection, recoil, rollback and persistence tests are part of acceptance.

## Remaining acceptance review

1. Finish the cockpit command comparison, including aliases and argument grammar.
   The controlled `PRONE` action is implemented below. `SAFETY` controls the
   separate MechWarrior player-killer preference, not friendly-fire safety; its
   combat target class is deferred. Shared `LRS`/`lrsmap` aliases and native/Lua
   firing coordinates and mechanical admission are implemented below. Finish
   remaining hiding interactions, preparation ordering, and numeric edge cases.
   Shared weapons-hold command admission is implemented below; the separate
   low-level damage warning now covers physical attacks, domino collisions and direct
   material weapon impacts, including vehicle critical cascades. Remaining special
   damage paths still need review. Shared `HIDE` is implemented for supported
   camouflage units and wizard operators; aircraft crash and transfer ordering are
   verified below. Invisible/clairvoyant operator state and its visibility, sensor,
   terrain-display and hiding interactions are implemented below.

   The weapon-catalogue comparison now has simulated implementations for
   IS MML 3/5/7/9, Clan ATM 3/6/9/12 and Clan Streak LRM 5/10/15/20, with
   acceptance recorded below, including MML LRM special rounds. The 25 non-simulated weapon stock entries are personal or infantry
   equipment within the deferred unit scope. Construction of shipped templates
   alone does not prove combat parity.

2. Review operator commands, Lua functions, configuration consumers and help against
   their reference catalogues. The cockpit catalogue is only one interface boundary.
3. Reconcile movement, sensors, combat, character progression, maps/environment and
   persistence contracts with the existing tests. Audit both Mechs and vehicles;
   do not infer cross-chassis coverage from a single fixture.
4. Exercise complete supported scenarios through commands and Lua, including
   shutdown/restart, callback rollback, casualties, cleanup and observer messages.
5. Refresh construction audits and the full suite after the remaining work. Update
   readiness markers only when their documented meaning is actually established.

The current `simulation_supported=false` vehicle field is documented as a full
parity marker. It must not be changed solely because movement and firing work.

## Heat cutoff characterization

The read-only reference `unit/mech_tic.c` schedules an absolute enable/disable
choice after four seconds, rejects a second pending toggle and checks the
configuration gate before cockpit authority. The command does not require a
running reactor or map placement. The event itself flips the flag and emits a
cockpit notice without repeating command admission.

In `movement/mech_update_heat.c`, cutoff runs after water, inferno and temperature
adjustments. It compares stored weapon heat plus continuous production minus
dissipation with the 9–10 band:
above or at 10, enable `floor(overheat - 10) + 1` cooling points; below 9,
disable `floor(9 - overheat) + 1`. Each sample changes at most two cooling points,
or four with double/Clan heat technology, bounded by disabled/active capacity.
Turning cutoff off restores cooling at that same bounded rate rather than all
at once. Enabling/disabling changes the current sample's dissipation directly;
water bonuses are recomputed on the next sample. Existing Rust thermal activity
filtering must account for pending toggles and disabled capacity even when stored
heat is zero. Tests must cover those idle transitions as well as submerged,
inferno, temperature, damaged sink and shutdown cases.

## Preferred ammunition characterization

`unit/mech_advanced.c::mech_usebin` accepts a weapon number and section (or `-`
to reset), requires a conscious assigned pilot and map placement (not a running reactor), and rejects
unavailable/recycling weapons, energy weapons and absent/destroyed sections.
It does not require a matching bin in the chosen section when the preference is
set. The preference is attached to the weapon's primary critical.

`unit/mech_weapons.c` searches the preferred section first when weapon preferences
are enabled for a lookup, then the mount's section, then other sections in
canonical order. It preserves ammunition compatibility/mode filtering and falls
back if the preferred section has no usable rounds. The Rust shared feed planner
already implements mount-first fallback; extend its ordering key and the two
chassis state adapters rather than duplicating launcher or expenditure logic.
Audit every ammunition lookup consumer, including automatic defenses and failure
paths, before claiming this feature complete.


The preferred-section implementation preserves the explicit lookup distinction:
`combat/mech_combat_missile.c` does not enable weapon preferences for automatic
missile defense. Its mount-first selection is unchanged. Ordinary configured
firing, expenditure and unjam paths use the shared preference-aware feed planner.


## Idle heat cutoff correction

The countdown now completes independently of thermal samples. An idle stopped
unit keeps intentionally disabled cooling and its thermal-check clock unchanged;
regulation and gradual restoration resume when accounting is active. Focused
lifecycle tests cover both enabling and disabling while stopped, then starting
the reactor. The server restart test verifies an idle toggle finishes even with
new commands disabled by configuration.

## Automatic turret characterization

`unit/mech_advanced.c::mech_auto_turret` toggles immediately, ignores trailing text,
and requires a conscious assigned pilot, map placement and surviving turret;
reactor power is not required to select the mode. The per-second heartbeat calls
`movement/mech_update_piloting.c::mech_turret_autoturn_update`, which does require
power, consciousness, vision and an unlocked/unjammed surviving turret. With a
unit or hex target selected, it sets relative turret bearing directly to world
bearing minus hull heading and marks visibility dirty. It does not slew gradually.
Audit target cleanup and cross-map validity before reproducing the target lookup.


## Reactor blast and sensor-flash delivery

`reactor_explosion_action` and trusted callback `btech.unit.reactor_explode(unit)`
now destroy a Mech and apply its radial blast through shared Mech/vehicle packets.
The center receives `max(tons/5, engine_rating/10)` damage and
`max(tons/10, engine_rating/25)` heat; neighbors within two hexes divide both by
`distance + 1`. Packets contain three damage points. The reactor mode selects
ordinary or punch hit locations. Per-cell altitude bounds are exclusive at ground
minus five and ground plus three, including underwater ground. Facing uses the
continuous explosion point. Neighbor woods ignite through the same helper as
mine blasts; source woods are not automatically ignited.

The source is raised six levels in a transient visibility snapshot after hull
loss. Visible observers with infrared or light amplification as their primary
sensor receive four seconds of blindness, unless already blinded or unconscious.
Secondary susceptible sensors alone do not trigger it. Saved blindness suppresses
ordinary controls, cockpit readouts, visual broadcasts, spotting and automatic
turret tracking. It does not erase contact ownership or stop motion. Recovery is
independent of power and placement and wakes the server heartbeat. Trusted state
inspection reports `blinded_remaining`.

The host action checkpoints destruction, map fire, all target damage, sensory
state, notices and casualty departure callbacks together. Ejected MechWarrior
infantry construction remains deferred with infantry; fatal occupants use the
existing casualty/afterlife path. This is not evidence for ejection-system parity.

Focused tests cover all seven supported chassis as blast recipients, both Mech
chassis as sources, radius and altitude boundaries, both susceptible sensor modes,
primary/secondary selection, existing blindness and unconsciousness, forest
ignition, deterministic replay, callback rollback, casualty departure rollback,
save/load and actual idle-server recovery. Existing mine and turret tests cover
their shared callers. The full suite passes 1,880 tests; formatting and Clippy also pass.

## Self-destruction and engine instability

Native `EXPLODE` and Lua `btech.unit.explode` now share admission, a durable
countdown, scheduling order and the existing reactor/ammunition damage actions.
The heartbeat processes idle timers and restores world state and notifications
when a database commit or casualty callback fails. Configuration gates, stop,
case-sensitive wizard override, cooldown checks and ammunition protection are
implemented. In-character units restrict ordinary control to their assigned
pilot; wizards and occupants of out-of-character units retain ownership exceptions.
Engagement releases the assigned pilot.

The actual countdown preserves the reference's untagged-delay behavior: an ammo
request advertises half the configured delay, but a normal Mech countdown selects
a reactor blast; delays above 256 select the event's ammunition mode. Boundary
tests cover signed settings, modulo-256 timing, live ammunition cancellation and
stop/override controls. Vehicles share the command and scheduler, with their own
rear-section destruction outcome. Destruction selects terrain support before the
six-level wreck offset; descent depends on the original altitude relative to the
upper surface. This includes bridge beds, water and ice. Falling ground vehicles
keep the server heartbeat active even when destroyed and without crew recovery;
a live-server reload test verifies that an otherwise idle wreck reaches ground.

The final four-point injury uses the shared tactical crew machinery after the
pilot assignment is cleared. Existing character injury counts seed that terminal
counter without damaging the evacuated player's personal health. A new wreck
recovery timer can survive restart; destruction clears the previous recovery and
sensor blindness first. Cross-chassis, authority, callback rollback and live-server
commit-retry tests cover these contracts. The optional section-loss stackpole
trigger now reuses the same reactor resolver, as described below.


Tracking refinement: crew stun does not suppress automatic turning; actual pilot
or unit-crew unconsciousness does. The desired integer world bearing is converted
using the continuous Rust hull heading, preserving that bearing during fractional
turn steps. Final focused tests verify both distinctions.


Self-destruct validation: the full suite passed 1,889 tests. The final heartbeat
change also passed all 27 focused self-destruct, reactor and VTOL crash tests.
Formatting, all-target Clippy with warnings denied and diff checks pass. The
reference tree remains unchanged. This milestone does not close the broader
acceptance review above.

### Engine-instability implementation and ordering

`combat/mech_armor_damage.c` records the event tick when positive internal damage
first reaches an otherwise pristine center torso. This happens after that hit's
critical dispatch and before internal structure is subtracted. Bulk engine loss
in `combat/environment_damage.c::mech_parts_destroy` checks the stackpole setting
when losses reach three, an inclusive 30-second window, a 2d6 roll of at least
nine, and a running reactor or
pending startup. The random roll precedes the power check in its short-circuit
condition. The shared section-loss and flooding paths now implement this ordering
and retain nested reactor reports for the existing casualty publication path.
A stopped reactor still consumes an eligible roll, while a disabled or expired
check does not. Ordinary engine criticals in `crit_mechs.c` do not perform this
check. Supercharger overload and direct administrative slot destruction also
stay separate. A pristine core hit opens its window after ordinary criticals
but before section destruction removes its engine parts.

A bounded initial-world clock preserves the zero-initialized timestamp's first
30 seconds. A per-Mech window takes over after pristine center-torso internal
damage; it is opened after critical dispatch, never before. Thirty elapsed
seconds remain eligible and the next tick expires the window. Both clocks
advance on otherwise idle server ticks and persist atomically with the world.
The initial clock uses the native `btech_reactor_clock` extension; runtime policy
still comes from `stackpole` and `explode_reactor`. Lua unit state exposes the
optional `reactor_instability_remaining` counter.

Focused tests cover the distinction between ordinary criticals and bulk engine
loss, flooded engine compartments, initial and damage-window boundaries, stopped and starting
reactors, configuration gating, deterministic replay, persistence validation,
ammunition-triggered blasts affecting neighboring vehicles, casualty callback
rollback, and an idle server retrying a failed clock/database commit.


Manual reactor destruction now checks the same bulk-parts instability path as
combat destruction. An eligible running/starting reactor can complete one nested
blast while its own engine compartment is destroyed, then finish the originally
requested blast. Engine losses are already recorded before reentry, so the nested
call cannot keep retriggering itself. Reports expose `section_explosion` and keep
its casualty effects inside the enclosing transaction. Focused tests cover the
roll threshold, startup/damage windows, stopped and starting power, the two crew
injury applications, and deterministic replay.

A neighboring reactor chain is also covered: damage from the first reactor can
destroy a second reactor's engine compartment, whose blast reaches the original
wreck and a vehicle. Existing engine losses prevent a cycle. Tests verify the
nested damage reports, callback-failure rollback, casualty evacuation, and saved
state replay.


Instability validation: the final full suite passes all 1,896 tests. All-target
Clippy with warnings denied, formatting and diff checks pass. Existing server
retry expectations include the new committed clock; flooding and vehicle-shot
regressions retain the new roll ordering and possible blast feedback. The
reference tree is unchanged. The cleanup implementation is described below;
the broader cockpit/operator review remains open.


### Delayed cleanup of fully destroyed in-character units

A shared native countdown retires fully destroyed in-character Mechs and vehicles
on the tenth committed second. Ordinary destruction with surviving structure does
not qualify. Admission clears radio channels and TICs and uses the existing
casualty evacuation transaction. Changing the IC flag later does not cancel the
event. Timers present at heartbeat entry advance before new combat casualties,
so newly admitted wrecks retain their full delay.

The departure action sees the old BattleTech identity. Retirement then removes
native construction and registration, detaches targeting and towing, sets Going,
Dark and Zombie, and quietly teleports the surviving game object to
`usedmechstore`. The unit's move callback still runs. Source teleport-out policies
remain active; a denied or redirected move rolls the action back. Mines and other
data owned by the surviving game object remain intact. This is separate from
ordinary object purge, which removes object-owned data too.

The optional native `btech_wrecks` table stores countdowns. Timer updates, native
record removal, flags, movement and callback effects share the host's save and
rollback boundary. The server keeps ticking for an otherwise idle wreck and
retries a failed commit. The broader cockpit/operator acceptance review remains
open.


Reactor-chain validation: all 1,898 tests pass, including manual reentry and
neighboring-reactor rollback/replay. All-target Clippy with warnings denied,
formatting and diff checks pass. The reference tree remains unchanged. The broader cockpit/operator acceptance review remains open.


Wreck-cleanup validation: all 1,903 tests pass in `target/wreck-final-full.log`.
All-target Clippy, formatting and diff checks pass; the reference tree is
unchanged. Public-boundary follow-up now verifies that an ordinary blast on an
already fully destroyed unit does not schedule cleanup or restart a pending
countdown. The reference `mech_damage_apply` skips or transfers through lost
sections and returns before invoking section destruction. Its double-section
cleanup guard is not an ordinary-hit trigger; applying it to every blast packet
would change behavior. The regression covers all seven supported chassis.

### Controlled drops

Native `prone` and Lua `btech.unit.prone(unit, pilot)` share one action for bipeds
and quads. The action requires a conscious assigned pilot in a running, grounded
Mech with no pending stand. It ignores trailing positional arguments, as the
reference command does. Vehicle movement families reject the Mech-only posture.

Speed thresholds use one-third and two-thirds of the shared effective maximum,
including live load, boosters, myomer and map conditions. Slow drops consume no
control dice and cause no fall damage. Faster drops use the shared control/XP
check, with a two-point modifier at running speed. Failure invokes ordinary fall
and injury resolution at severity one or two. Successful controlled drops retain
a pending heading while stopping translation. The common posture mutation
centers the torso, unflips arms and cancels hull-down state.

Flooding, water inferno extinguishing, configured stagger-history clearing and
stepping mines finish the sequence. A failed fast drop can first trigger ordinary
fall mines. Existing casualty and notification publishers own nested effects;
callback failure restores posture, damage, dice, XP and notices together.

The source's `mech_stagger_level` reads an independent runtime scalar, not its
rolling damage list. Source writes initialize/reset it to zero or set it to -10;
ordinary fresh units never acquire a positive value. The Rust action therefore
does not invent an additional drop modifier from its damage history. A focused
regression verifies a stationary drop with forty points of queued stagger damage
remains roll-free and clears that history in configured rolling mode.

Focused tests cover both Mech anatomies, exact speed boundaries, legal reverse
travel, deterministic success/failure replay, native/Lua parity, restart,
posture-control reset, admission rejection, vehicle rejection, water flooding and
IC evacuation, and stepping-mine callback rollback. Validation is recorded below.
The broader cockpit/operator acceptance review remains open.


Controlled-drop validation: `target/prone-full.log` passed every gameplay and
other test target; its only failure was the strict expected native-command list.
The corrected catalogue target passes all 17 tests in `target/prone-catalogue.log`.
Together these verify all 1,910 current tests. `target/prone-final-clippy.log`,
formatting and diff checks pass. The reference tree remains unchanged.

### Long-range command interface

Native `LRS` and `lrsmap` resolve through one command registration and renderer.
All seven display modes accept case-insensitive first-letter selection, while Lua
also retains descriptive mode names. Native and Lua requests share cockpit,
hardware, centering and mode validation in that order. Tactical and navigation
centering use the same admission-first parser; navigation retains its existing
hardware exemption. Integer bearings and finite signed decimal ranges are checked. The reference
uses `strtof`, so hexadecimal floats and exact float32 range/rounding boundaries
still need a shared numeric-parser audit; decimal examples alone do not prove
complete numeric compatibility.

The mixed-map regression exercises every supported chassis, all seven modes,
and own-unit, bearing/range, dbref and contact-label centers through native aliases
and Lua. It also verifies rejection ordering and unchanged state for absent
pilots, stopped reactors, blindness, destroyed Mech scanners and invalid arguments.
Focused map, scan, access and command tests pass (91 tests). Shared
`FIRE`/`FIRETIC` target grammar remains open.

Firing reference evidence: `mech_fire_command.c` checks the observer branch
before `weapon_fire_target_resolve`. An IDF weapon with a selected observer and
no firer unit lock invokes `mech_spot_fire` regardless of explicit arguments.
Only requests that reach target resolution use explicit-unit LOS or explicit X/Y
coordinate handling. Coordinate shots select an occupant for non-artillery
weapons, otherwise address the hex; artillery has its own LOS behavior.
`mech_tic.c` forwards the same arguments per weapon. The Rust adapters accept
contact IDs, dbrefs and explicit X/Y coordinates. Native target arguments now
remain undecoded until per-weapon dispatch; cockpit authority precedes decoding.
The full mechanical rejection ordering still requires review. Preserve observer precedence, TIC
recoverable shot failures and fatal callback rollback in that work.

Long-range validation: `target/lrs-full.log` passed 1,910 tests and found one
heavy-Gauss recoil fixture failure: cleanup tried to edit TICs after the pilot
could become unconscious. The fixture now creates identical groups before firing
for every replay and makes no post-fall cockpit edits. Gameplay code did not
change after the full run. All 350 motion tests pass in
`target/lrs-motion-final.log`, completing verification of all 1,911 current tests.
All-target Clippy (`target/lrs-final-clippy.log`), formatting and diff checks pass.
The reference tree remains unchanged. Numeric edge cases and the firing interface
work identified above remain open; this does not close the overall delivery goal.

### Shared native firing identities

`fire` and `firetic` now share one optional-target parser and the existing
battlefield identity lookup. Both accept case-insensitive two-character contact
IDs (including the reference's ignored suffix) as well as explicit dbrefs. The
parser does not change the default lock. Shot admission and publication remain
in the common configured firing transaction.

The TIC integration scenario now covers all seven supported chassis, compares
single shots and groups through uppercase/lowercase/suffixed IDs and dbrefs with
Lua results, and verifies invalid identities and surplus arguments preserve state.
It retains disabled-weapon/recycle rejection, callback rollback and persistence
coverage. Validation is recorded below.

Native coordinate targets and deferred argument decoding are now implemented
below. Source mechanical admission/error ordering still requires review.
Observer precedence is verified below. Coolant recipients are resolved once by the host without changing a unit's
saved lock or spotter.

Firing-identity validation: the full run (`target/firing-labels-full.log`) passed
1,907 tests with four failures. Two fixture assumptions were corrected without
changing gameplay code: wreck excess heat is a delayed sample rather than a
monotonically decreasing value, and the sensor-dice test now fixes target-owned
impact randomness. The corrected duel and all 350 motion tests pass in
`target/firing-fixtures-final.log`. The two scheduling failures (a live database
byte comparison and an in-flight cleanup observation) did not reproduce; all 13
schedule tests pass in `target/firing-schedules-rerun.log`. Together these runs
exercise all 1,911 current tests successfully, but the scheduling timing failures
remain an open test-reliability issue rather than a proven fix. All-target Clippy
(`target/firing-labels-final-clippy.log`), formatting and diff checks pass.

Fresh construction audits (`target/firing-template-audit.json` and
`target/firing-vehicle-audit.json`) confirm 1,299 of 1,302 Mech templates and all
269 supported vehicle templates construct. Daishi-H constructs; the only parsed
Mech failures are LAM equipment in PHX-HK2, STG-A5 and WSP-105. The reference tree
remains unchanged. Overall integration is still incomplete.

### Coolant target intent and observer precedence

Configured native/Lua shots resolve automatic coolant self-selection once
before calling either shot engine. Both engines honor the resolved coolant
recipient. Heat-mode coolant selects the carrier
only when the target is omitted. Explicit recipients retain direct-shot admission,
while an explicit self recipient still receives coolant normally. One shared
policy determines automatic self-selection; TICs reuse the same host transaction.
Rust tactical APIs take an explicit recipient; heat mode no longer replaces it.

The complete reference handler confirms that IDF observer dispatch precedes
argument target resolution when the firer has no unit lock. This overrides even
an explicit recipient. Mixed Mech/vehicle regressions compare omitted and explicit
recipients with the firer facing away and lacking acquired contacts. Coolant
regressions cover explicit other/self recipients, omitted targets, coordinate locks,
expenditure, heat, callback rollback and native/Lua single/TIC equivalence.
All 1,911 tests pass in `target/fire-target-final-full.log`. The initial run also
exposed a surface fixture that mixed primary artillery height limits with random
secondary reactor blasts; it now uses stopped reactors. All 104 surface tests pass
with that correction in `target/fire-target-surfaces-final.log`. All-target Clippy
(`target/fire-target-final-clippy.log`), formatting and diff checks pass. The
reference tree remains unchanged. Previously observed scheduling flakiness was
not reproduced by this run and is not claimed fixed. Coordinate argument handling,
numeric edge cases remain open; see the coordinate interface work below.
Integration is not complete.

### Shared coordinate firing interface

Native `fire <weapon> <x> <y>` and `firetic <groups> <x> <y>` use the same
per-weapon request dispatcher as unit IDs and dbrefs. Lua `btech.unit.fire` and
`btech.unit.tic_fire` accept an `{x, y}` target table. Rust TIC callers can pass
`BattleFireTarget::Hex`; optional unit arguments still express omitted or explicit
unit selection. No request changes the saved unit or hex lock.

Conventional coordinate requests select the first eligible occupant in shared
map-slot order, retaining coordinate-directed feedback and damage metadata. Empty
coordinates use the existing terrain resolver and saved hex mode/bonus. Artillery
always queues a coordinate impact, ignores occupants for immediate damage, and
accepts explicit coordinates with no lock or a different unit/hex lock. Its explicit
coordinate path retains the reference's LOS/spotter exemptions; normal selected
and observer-directed artillery retain their existing admission.

Native targets are decoded inside each weapon's attempt, after cockpit admission
and weapon lookup. Argument-count checks precede IDF observer dispatch; a selected
observer with no firer unit lock bypasses otherwise-invalid identity/coordinate
arguments. Mixed TICs still reject non-IDF weapons and fire eligible IDF mounts.
Shot rejection is recoverable, while callback/publication failure restores the
whole transaction. Shared mechanical rejection before target parsing is now
implemented in the admission work below; outer gates and later preparation still
require acceptance review.

New tests cover all seven supported chassis, occupied and empty coordinates,
mode-dependent empty-hex bonuses, unchanged locks, native/Lua/Rust TIC equivalence,
callback rollback, invalid coordinates, absent pilots and restart. Artillery tests
cover Mech and vehicle carriers with absent, unit and hex locks. Mixed observer
regressions include invalid arguments and IDF/non-IDF TIC members. Focused tests
pass; full validation results are recorded below. Overall integration remains incomplete.

Mechanical-order evidence: `mech_fire_command.c` checks stun, temporary weapon
failure, recycling, prone support, cover restrictions, destroyed mounts and
defensive-only weapons before target argument resolution. Ammunition checks
occur later in fire preparation. The shared admission implementation below
separates those phases without moving the whole `BattleWeaponReadiness::ready`
boolean before target decoding.

The source also distinguishes self-spotter rejection before weapon lookup from
non-IDF observer rejection after mechanical and argument-count checks. These now
use separate positions in the shared dispatcher. The outer native reference
command's weapons-hold guard and hiding side effects remain acceptance items.

### Surface-collapse casualty ordering

The coordinate-interface full run exposed a production interaction, not merely a
random fixture failure. A JR7-D's fall can breach its center torso, flood the
engine and detonate its reactor. A second occupant already selected for a fall
can be destroyed by that blast. Repeating ordinary fall admission then rejected
that wreck and rolled back the entire fracture. A deterministic seed of 14
reproduces the failure; a diagnostic sweep of all 256 repeated-byte seeds passes
after the correction.

The shared fracture transaction now finishes falls admitted before the collapse,
including wrecks created by earlier falls. Mechs expose an internal material
resolver analogous to the existing vehicle resolver; ordinary public fall entry
points retain their destroyed-unit guard. The reference's `swim_except` and
`mech_fall` also finish environmental falls without a new destruction rejection.
The transaction still owns terrain, damage, dice, notices and final validation.

The new regression checks both trigger orders, an additional VTOL casualty,
reactor provenance, completed wreck falls, ordinary wreck rejection, and identical
reports/state after save/reload. Its two-level ice depth keeps the aircraft on the
surface inside the reactor blast's exclusive height bound; the original three-level
fixture placed it above that blast. Temporarily restoring the vehicle admission
in a control run reproduces `Vehicle is destroyed`; the corrected shared resolver
passes. The prior database-failure test retains its full rollback assertions.

Coordinate fixture acquisition now receives its fixed combat dice before scanner
refresh. Previously, the fixture seeded only after acquisition, allowing an
unacquired target to fail an otherwise equivalent native/Lua firing comparison.
This changes test setup, not the contact-acquisition rules.

Validation: `target/fracture-cascade-full.log` ran all 1,915 tests, with 1,913
passing and the two fixture failures above. After their correction, all 108 tests
in `btech_fire_targets` and `btech_surfaces` pass in
`target/fracture-cascade-final-targets.log`. Production code is unchanged from
the full run; the temporary negative control was restored before final validation.
All-target Clippy with warnings denied (`target/fracture-cascade-final-clippy.log`),
formatting and diff checks pass. The reference tree remains unchanged. Integration
is still incomplete; firing admission ordering and the wider acceptance review
remain open.

### Shared weapon mechanical admission

`WeaponMechanics` owns the ordered pre-target checks for both chassis stores.
Mechs supply their limb recycle, carried club and prone-support facts; vehicles
supply temporary weapon failures and dug-in cover. Both supply stun, physical
mount integrity and weapon recycling. Nonfunctional mounts skip recycle lookup
rejections, matching the reference lookup's destruction precedence. Mechanical
feedback precedes target argument-count and decoding errors in ordinary shots,
terrain fire, artillery and TIC attempts.

The dispatcher rejects self-spotting before weapon lookup and preserves the later
non-IDF observer restriction. Defensive-only rejection follows mechanical checks;
AMS remains mechanically ready for automatic defense. Ammunition, spent launchers,
feed jams and ongoing preparation work remain in their existing later phase.
Readiness inspection reuses the same mechanical calculation, including the
previously missing carried-club restriction. Beam versus ammunition recycle
wording reuses the catalogue skill family; all 141 matching reference catalogue
entries agree on that classification.

Four new integration tests cover all seven supported chassis plus Mech/vehicle
artillery carriers, single native/Lua fire, native/Lua/Rust TICs, invalid targets,
recycling versus stun/temporary failure precedence, destroyed mounts, self-spotting
before invalid weapon lookup, empty supply, club and support restrictions, cover,
AMS automatic-defense readiness, unchanged state/effects and saved replay.
Existing command tests now expect the specific recharging message on a second
shot. All 1,919 tests pass in the full run (`target/weapon-admission-full.log`).
All-target Clippy with warnings denied (`target/weapon-admission-clippy.log`),
formatting and diff checks pass. The reference tree remains unchanged.

This closes mechanical admission for the currently represented state. It does
not establish complete command parity: weapons hold, hiding/reveal side effects,
remaining preparation ordering and numeric argument edges remain under review.

Next outer-gate evidence: `commands/mech_command_catalog.c` registers `HIDE` with
an unrestricted type mask. `combat/bsuit_hide.c` permits a unit with `CAMO_TECH`
or a wizard actor before applying the normal battle-armor/MechWarrior limitation.
It therefore includes supported Mechs, ground vehicles and VTOLs. It requires
ordinary running cockpit admission, no jump/out-of-control state, speed at most
one movement point, no pending hide event, and a landed VTOL. Supported cover is
light/heavy forest, mountains or rough terrain; the building exception belongs
to deferred battle armor. Each second checks hostile running surviving observers
(excluding clairvoyant/observer/invisible units) for LOS and rejects any height
above the surface. Ordinary supported units use five turns, VTOLs four; absence
of camouflage doubles those values. `HIDE_TICK` is 10 and the first scheduled
event starts at counter zero, so completion follows the inclusive counter check.
Movement, damage and attempted firing have reveal/cancellation consumers to audit.
The shared Rust controller now implements native `hide`, Lua `btech.unit.hide`,
durable elapsed events and heartbeat progression for all supported chassis. Separately, reference `status2` is exposed
as a bitvector in `scripting/value_catalog.c`, so the weapons-hold read cannot be
dismissed merely because no dedicated setter uses its named constant.


### Shared hiding controller

`hiding.rs` owns camouflage admission, cached hostile-observer checks, event
progression and cover loss. Chassis-specific access is confined to common timer,
signature and elevation facts. `Camo_Tech` is accepted by Mech construction.
With camouflage, Mechs and ground vehicles finish on the 51st one-second check,
and VTOLs on the 41st; without camouflage a wizard takes 101 or 81 checks.
Visibility checks reuse acquired contacts without rerolling detection.

Actual hex crossings cancel preparation and reveal cover; motion inside one hex
does neither. Positive armor damage reveals cover while retaining preparation.
Shutdown cancels preparation. Admitted firing attempts reveal and cancel even
when mechanical or target checks reject the shot. Native rejection commits that
intent; an aborted Lua callback restores the entire callback transaction.

Eight focused tests pass in `target/hiding-extended.log`, covering all seven
supported chassis, both interfaces, camouflage and wizard eligibility, timing,
restart, cached observers, speed and elevation gates, damage, shutdown, movement,
rejected fire, callback rollback, failed-save retry and idle server completion.
Command/access catalog tests also pass in `target/hiding-focused.log`.

Remaining parity work includes cover-loss ordering when an aircraft crosses a
hex and crashes during the same event, map-transfer interactions, and the
reference clairvoyant/invisible observer flags, which are not represented by the
existing observer-mode flag. Weapons hold remains a separate outer firing gate;
its reference status bit is operator-writable and must not be treated as dead
state. The full integration remains incomplete.


Weapons-hold ordering evidence: `mech_fireweapon` and `mech_firetic` check the
condition after ordinary cockpit admission and before parsing their arguments.
That gate therefore precedes HIDE cover loss. In `mech_damage.c`, the lower-level
attacker weapons-hold check emits feedback but does not return; it must not be
implemented as unconditional damage suppression. The operator-state interface
and ordinary shot/TIC admission need shared typed state and separate tests for
these two behaviors.


Hiding validation: `target/hiding-full.log` executes all 1,927 tests, with 1,926
passing. `tcp_admission_controls_cache_and_existing_queue` fails its raw database
byte comparison at `tests/schedules.rs:668` while a live server is running.
The unchanged complete scheduler target then passes all 13 tests in
`target/hiding-schedules-recheck.log`; this is evidence of intermittency, not a
fix. All-target Clippy with warnings denied passes in `target/hiding-clippy.log`.
Formatting and diff checks pass, and the reference tree remains unchanged.


### Aircraft crash cover ordering

Aircraft contact resolution now runs the shared movement reveal immediately
after placing a crash in a different hex, before shared falling damage. Its
notices precede the fall's armor feedback in the same report. Crashes within the
original hex retain damage-driven reveal. The existing enclosing candidate and
host transaction preserve atomic rollback; there is no aircraft-specific hiding
controller.

The new cross-hex versus same-hex regression test verifies the source of cover
loss, pending-event cancellation, notice order, saved replay, live movement and
restoration after failed final validation. Temporarily removing the production
change makes the crossed-hex case report damage-driven reveal instead of
movement-driven reveal (`target/hiding-crash-negative.log`). After restoring it,
all 24 hiding and aircraft-crash tests pass in `target/hiding-crash-focused.log`.

Transfer audit evidence: explicit building entry calls `mech_position_set`,
which resets both current and previous coordinates through `mech_position_xy_set`.
Ordinary `move_mech` captures previous coordinates, runs the edge transition,
then checks map/hex changes for cover loss. Do not apply a blanket reveal to
every administrative or building relocation. Explicit entry, movement-driven
exit, towing and callback interactions still need dedicated acceptance coverage.


Aircraft crash validation: the full run passes all 1,928 tests in
`target/hiding-crash-full.log`. All-target Clippy with warnings denied passes in
`target/hiding-crash-clippy.log`; formatting and diff checks pass. The reference
tree remains unchanged. This closes the verified aircraft cross-hex crash ordering
gap, not the broader integration or transfer/observer acceptance review.


### Cover during building transfers and towing

The shared movement fallback now excludes units whose positions were mirrored
by a tow relationship. Reference `mech_towing_position_update` only mirrors the
target's position and heading, after the carrier has resolved its own movement;
it does not run the target's cover-loss event. Armor damage can still reveal a
towed target through the ordinary shared damage handler.

A regression scenario covers all 42 combinations of six mobile carriers and
seven supported tow-target chassis. Explicit entry preserves both units' cover
and pending hide timer. A subsequent movement-driven edge exit reveals and
cancels preparation only for the carrier. Failed arrival callbacks restore both
units and discard notices; saved replay produces the same result. Fixtures use
ordinary towing equipment to satisfy mass limits, rather than bypassing load
rules. All combinations pass in `target/hiding-transfer-focused.log`. Removing
the production exclusion reproduces the target's incorrect reveal/cancellation
in `target/hiding-transfer-negative.log`; the exclusion was restored afterward.

Further operator flags, weapons hold and the remaining command/scenario audit
are still required. These transfer checks do not establish full integration.


Transfer-cover validation: all 1,929 tests pass in
`target/hiding-transfer-full.log`. All-target Clippy with warnings denied passes
in `target/hiding-transfer-clippy.log`. Formatting and diff checks pass, and the
reference tree remains unchanged. Broader integration remains incomplete.


### Shared weapons-hold command admission

`weapons_hold.rs` owns a durable operator restriction and common firing admission
for both chassis stores. Native fire/TIC dispatch checks cockpit authority,
startup and placement first, then hold, then argument parsing. Lua/Rust firing
and TIC actions use the same admission before cover loss or weapon/target
resolution. Hold rejects an entire TIC request, including empty or invalid group
selections, without changing timers, cover, ammunition or random streams.

Wizard control is `@btech unit-weapons-hold <unit>=on|off`; trusted transactional
Lua reads or writes `btech.unit.weapons_hold(unit[, enabled])`. Unit state exposes
`weapons_hold`, and cockpit information displays `WEAPONS HOLD`. Mechanical
readiness remains independent. The restriction survives shutdown and persistence;
callback abort restores edits and failed admission leaves the world unchanged.

Two focused tests pass in `target/weapons-hold-focused.log`. They cover all seven
supported chassis, native/Lua controls, single fire and native/Lua/Rust TICs,
malformed arguments, cover preservation, readiness, status, shutdown/restart,
unauthorized native edits, invalid/Going subjects and cockpit/startup precedence.

The reference lower-level damage check emits an attacker warning without an
early return. That separate feedback path is not implemented by the command gate
and remains required, along with observer flags and the broader acceptance audit.


Weapons-hold admission validation: all 1,931 tests pass in
`target/weapons-hold-full.log`. All-target Clippy with warnings denied passes in
`target/weapons-hold-clippy.log`. Formatting and diff checks pass; the reference
tree remains unchanged. Low-level damage feedback and broader integration remain
open as described above.


### Physical damage attribution and weapons-hold feedback

Accepted physical impacts now retain attacker identity in the shared Mech damage
cascade. The hold warning precedes target cover/material changes and repeats at
transfer or nested explosion entries. It never suppresses damage. Charge and
DFA packets retain attribution independently of character XP eligibility;
self-inflicted recoil and falls do not warn. Dumped-ammunition ignition temporarily
uses self attribution, then restores the interrupted incoming attack's source.

Three focused tests pass in `target/hold-damage-focused.log`: ordinary kicks,
armor/internal transfers, transfer-triggered ammunition explosions, charge and
DFA target versus self packets, restart replay, and native/Lua publication and
callback rollback in tactical and character modes. After removing only the new
warning notices, complete reports and final world state match the unheld baseline,
including dice, criticals, injuries and recovery state.

Reference blast attribution is deliberately different: `artillery.c` passes the
victim as both target and attacker for each area-damage packet. Do not attribute
those packets to the original artillery shooter merely because the queue retains
its ID. That ID remains relevant to correction feedback, not the hold warning.

Remaining damage attribution includes direct weapon impacts, other collision or
physical paths and their vehicle counterparts. The new physical path does not
establish complete weapons-hold feedback or overall integration.


Physical-attribution validation: all 1,934 tests pass in
`target/hold-damage-full.log`. All-target Clippy with warnings denied passes in
`target/hold-damage-clippy.log`; formatting and diff checks pass. The reference
tree remains unchanged. `mech_domino.c` also passes a distinct attacker for
collision damage; Rust `stacking.rs` still uses unattributed impact calls and is
a concrete next consumer of the shared context. Direct weapon and vehicle damage
attribution remain open.


### Direct weapon and domino damage attribution

Domino collisions and direct weapon impacts now retain their source independently
of character experience eligibility. Mech and vehicle shooters share the target
resolver and weapons-hold notice helper. Vehicle armor penetration stays in the
same damage entry; critical-triggered ammunition and weapon explosions carry the
incoming attacker into a new entry. Combat-safe vehicle routing returns before
hold feedback. Falls and area blasts retain self attribution.

The focused checks cover Mech energy/missile impacts, domino target versus self
packets, all five supported vehicle shooter classes against Mechs, both shooter
anatomies against all five supported vehicle target classes, safe routing, restart
replay, penetration alone and an advanced-table ammunition cascade. Held and unheld
attacks produce identical material state and dice; only hold notices differ.
The replay fixture reconnects its pilot after loading, preserving the intended
skill comparison without persisting the runtime connection flag.

This closes the direct-impact and domino consumers identified in the preceding
milestone. It does not prove overall command parity or complete integration.


Validation: all 1,939 tests pass in `target/vehicle-attribution-full.log`.
All-target Clippy with warnings denied passes in
`target/vehicle-attribution-clippy.log`. Formatting and diff checks pass, and
`btmux-khi` remains unchanged. The next acceptance review includes the observer
exceptions shared by HIDE, sensor visibility and map LOS: reference
`combat/bsuit_hide.c`, `sensors/mech_sensor.c`, `sensors/mech_los.c` and
`map/map_los.c` use invisible/clairvoyant state that Rust does not yet model.
This is a cross-domain requirement; adding an exception only to HIDE would not
complete it. Continue the cockpit/operator/configuration review alongside that
work; the scope exclusions at the top remain unchanged.


### Operator visibility across sensors, terrain and hiding

`BattleVisibility` stores independent invisible/clairvoyant flags on both anatomy
records. Native `@btech unit-visibility` presets and trusted Lua
`btech.unit.visibility` share the same setter and persistence representation;
`@btech inspect` and Lua unit state expose the flags. Native authority, invalid
Lua tables, late callback failure and restart are covered across all seven
supported movement classes.

Ordinary sensor queries suppress invisible signatures across all nine sensor
modes. The scanner drops an existing contact, while read-only contact displays
and broadcasts already honor the current flag. Clairvoyant visibility can
inspect same-map units without acquiring sensor roles, including invisible units.
It can also see terrain through obstruction, darkness, sensor restrictions and
range limits. Physical terrain reports remain unchanged. Terrain visibility and
primary/secondary sensor roles remain separate, including building contact rows.
Unacquired weapon aim retains the reference's 10,000 sensor penalty; visibility
privileges do not manufacture an ordinary firing solution.

Hiding ignores invisible and clairvoyant opponents alongside observer-role units.
The focused suite covers all 49 observer/target chassis combinations, positive
control signatures for all nine sensor modes, no acquisition rolls against
invisible targets, all-chassis blocked-terrain observations, the unacquired aim
penalty, native/Lua authority, detached tables, rollback and saved-state equality.

Reference boundaries: `mech_sensor_can_see` rejects invisible targets;
`mech_los_check` and `map_los.c` bypass visibility for clairvoyants without setting
sensor-acquisition bits. `mech_sensor_to_hit_bonus` returns 10,000 without an
acquired sensor role. `bsuit_hide.c` excludes both flags when checking opponents.

A separate existing gap surfaced in this audit: automatic mixed-class scanners
still restrict their sensor pairs to Visual and Light Amplification in
`scanner.rs::supported_pair`. Complete nonvisual ground-vehicle/VTOL queries and
remove that restriction once cross-chassis acquisition and targeting behavior is
verified. Do not treat the all-mode Mech test as proof of all-mode vehicle parity.


Final visibility validation: all 1,943 tests pass in
`target/visibility-final-full.log`; all-target Clippy with warnings denied passes
in `target/visibility-clippy.log`. The final 78 display/sensor checks also pass in
`target/visibility-final-displays.log`. Formatting and diff checks pass and the
reference tree is unchanged. The earlier full run caught the obsolete primary
sensor marker for clairvoyant terrain; the final run verifies the corrected
separation between visibility permission and sensor roles. Overall integration
remains incomplete pending the acceptance work above.


### Nonvisual automatic scanning across chassis

The automatic scanner no longer filters mixed-class pairs to Visual and Light
Amplification. Radar derives both endpoints from shared position/altitude facts;
active probes share range, interference and concealment logic while the equipment
lookup retains the different Mech and vehicle slot rules. Concealment facts are
shared with weapon aim through the scanner projection.

Stationary installations receive the reference's integer forty-percent range
extension for radar and all three probe families, alongside infrared,
electromagnetic and seismic modes. Range and fluctuating-signal helpers now live
in the common sensor module rather than the seismic implementation. Mobile probe
and radar arithmetic remains available through the existing pure evaluators.

Thirteen focused tests pass in `target/mixed-scanner-focused5.log`. New coverage
includes all nine modes across all 49 supported chassis pairs, natural ground and
aircraft target exclusions, automatic observer scheduling, acquisition and saved
random-state replay, stationary-versus-mobile reach, native/Lua switching and
callback rollback, probe-only contacts through terrain obstruction, targeting and
probe-loss fallback. The existing physical-sensor and vehicle equipment/server
persistence checks also pass. This closes the scanner restriction recorded in the
preceding visibility milestone; it does not establish complete integration.


Next acceptance gap: reference `mech_damage.c` checks the target's
`MECH_STATUS_COMBAT_SAFE` state and the attacker's map `BUILDFLAG_CSI` bit before
weapons-hold feedback or damage. Rust's vehicle APIs accept an explicit
`combat_safe` rule, but the live unit state and shared damage pipeline do not yet
apply those scenario facts. The existing building bit 2 (`is_complex`) currently
protects building damage only; it is distinct from entrance safety bit 8
(`is_safe`). Audit and integrate this across anatomy-specific damage paths without
conflating entrance policy with unit immunity.


The sensor pass also corrects VTOL launch countdowns: `aero_takeoff` leaves the
reference landed state set until `mech_continue_flying` at liftoff. Seismic
observers and targets now remain grounded during `Launching` and lose that
eligibility in `Airborne` or `Falling`. The focused sensor set passes all fourteen
tests in `target/mixed-scanner-final-focused.log`. The former vehicle-scanner
expectation that skipped nonvisual pairs is replaced by an available-role test:
a seismic/visual pair still acquires via visual when stopped-target seismic
sensing is disabled.


Final mixed-scanner validation: all 1,947 tests pass in
`target/mixed-scanner-final-full.log`, and all-target Clippy with warnings denied
passes in `target/mixed-scanner-final-clippy.log`. The focused sensor set passes
14 tests and the updated vehicle-scanner set passes five tests. Formatting and
diff checks pass; `btmux-khi` remains unchanged. Continue with the scenario-derived
combat-immunity gap above and the remaining interface/behavior acceptance audit.

### Scenario combat safety across chassis

Unit state now persists `combat_safe` for both anatomy families. Trusted Rust and
Lua setters and the Wizard `@btech unit-combat-safe` command share validation;
callback failure restores the prior setting. Lua state inspection exposes it and
Mech status displays COMBAT SAFE. The existing map building flag 2 is consulted
at damage entry using the attacker's map, with environmental self-damage using
the target's map. Entrance flag 8 remains independent.

The shared policy precedes weapons-hold feedback, HIDE removal, armor, internal
structure, stagger, crew injury and damage cascades. Suppressed incoming damage
notifies the distinct attacker that its efforts only scratch the paint. Launch
costs still apply. Unit immunity also short-circuits hit-table and random critical
selection, personal fall checks, fall packets, VTOL crash rotor destruction and
Mech breach flooding. Mech fall reports now use an optional avoidance check to
represent its absence without a fabricated roll or skill result.

Map protection begins after hit-location routing. It intentionally does not
suppress independent hit-table consequences such as VTOL rotor loss, personal
fall checks or flooding of an existing breach. Those require the unit flag.
Six integration tests pass in `target/combat-safe-final-focused.log`, covering all
49 chassis pairs, ordinary versus immune launches, safe-map versus entrance-bit
controls, attacker-only flags, save/reload, native authority, Lua rollback,
physical hits, self damage, critical selection, fall injury and breach flooding.
A unit test also checks source-map versus target-map attribution across both
anatomy families on separate battlefields. Full validation is recorded below
when complete.

Next verified catalogue gap: FIRESWARM and FIRESWARM1 are LRM ammunition controls
in `commands/mech_command_catalog.c:118`, not the excluded battle-armor SWARM
command. Rust's ammunition modes currently have no corresponding state or shared
retargeting path. The reference `mech_combat_missile.c:477` carries surviving
missiles through successive targets with ten visited-target slots, caps cumulative travel at effective range,
chooses the first eligible map-ordered target within 1.9 hexes of the last target,
checks visibility, avoids already visited and combat-safe units, and applies the
friend-or-foe filter for Swarm-1. Its visited-slot boundary is checked after an
attack; test the terminal iteration explicitly instead of relying on the source
comment about ten targets. Audit launch, ammunition grammar, AMS, cluster
arithmetic, target feedback and restart/rollback together before implementing a
shared path for Mech and vehicle launchers. Continue the broader interface and
behavior acceptance audit; this milestone does not establish complete parity.

Hit-table follow-up also remains: reference `mech_hit_location` consumes its
initial 2d6 before delegating to FASA or critical-proof routing, and the delegated
handler consumes another 2d6 before its combat-safe return. Vehicle routing
already models both draws. Mech `BattleHitRules::resolve` currently uses the
caller's roll directly, including when `fasa_criticals` is enabled. Audit that
mode's table distribution and saved random stream against the reference for
both ordinary and immune targets; the new standard-mode immunity checks do not
prove FASA or critical-proof Mech parity.


Final combat-safety validation: all 1,954 tests pass in
`target/combat-safe-final-full.log`; all-target Clippy with warnings denied passes
in `target/combat-safe-clippy.log`. The final six focused integration tests pass
in `target/combat-safe-final-focused.log`; the separate-map attribution unit test
also passes in the full suite. The initial full run exposed nondeterministic
contact acquisition in the new test fixture. Its signal and random state are now
seeded, and a shared setup helper explicitly establishes the contact before
resetting attack dice. No production behavior was changed to accommodate that
fixture failure. Formatting and diff checks pass; the reference tree remains
unchanged. Integration remains incomplete, including the hit-table and Swarm
missile gaps above and the broader acceptance work at the start of this file.

### Mech delegated hit-location routing

The hit-table follow-up above is implemented. `BattleHitRules::resolve` treats
the supplied roll as the entry to routing. FASA and `CritProof_Tech` consume one
additional 2d6 before selecting from shared rows or returning for combat safety.
Critical-proof precedence suppresses TACs and random component selection while
retaining damage, head injuries, critical-count rolls and internal roll-twelve
limb destruction. Biped and quad construction now accepts the technology flag.
No additional launcher-specific routing or duplicate table rows were introduced.

Eight focused tests pass in `target/mech-hit-routing-final-focused.log`. The new
matrix checks all 2d6 totals, arcs, both anatomies, FASA/critical-proof precedence,
head-graze modes, immunity, invalid-input atomicity and serialized random replay.
Configured native and Lua shots from both attacker anatomy families reproduce
state and hit feedback after save/load, and aborted callbacks restore the whole
action. Material tests distinguish component immunity from head injuries and
limb loss and retain both damage-stage rolls when a TAC is supplied explicitly.
Full-suite and lint validation follow below when complete. The Swarm ammunition
and broader interface/behavior acceptance work remain outstanding.

The first full routing run exposed thirteen expectations in the jump, motion
and vehicle-fire suites that used standard-table predictions or first-roll
head-hit seeds despite enabled FASA configuration. Direct/native comparisons now
use the same scenario hit settings, and casualty fixtures select the delegated
head roll. Conventional salvo checks retain packet-size assertions and explicitly
require target destruction when the salvo ends early. All 447 tests in those
three suites pass in `target/mech-hit-routing-existing.log`; the full rerun is
recorded below when complete.


Final Mech routing validation: all 1,957 tests pass in
`target/mech-hit-routing-final-full.log`. All-target Clippy with warnings denied
passes in `target/mech-hit-routing-final-clippy.log`; formatting and diff checks
pass and `btmux-khi` remains unchanged. The eight focused checks and 447 affected
existing checks also pass. This closes the Mech FASA/critical-proof routing gap
recorded above. Swarm missile ammunition and the remaining scoped acceptance
audit are still incomplete; the overall integration goal remains active.


### Shared Swarm missile flights

FIRESWARM and FIRESWARM1 now share one Rust control and retargeting resolver
across Mechs and vehicles. Typed Swarm/Swarm-1 supply, template validation,
weapon/status display, native commands, Lua controls and detached per-hop reports
are implemented. The existing launch transactions and per-anatomy damage paths
own expenditure, cluster rolls, hit routing, crew effects and XP publication.
Incoming missile caps are applied by the shared weapon-packet code before damage.

The caller audit corrects the earlier missile-helper-only interpretation:
`mech_hit_resolution.c:401` returns on an initial miss before starting the Swarm
helper. Only secondary misses retain their incoming missiles for retargeting.
The original adjusted hit threshold and glancing state apply to secondary attacks.
Retargeting uses the first eligible persisted map slot, retained contact visibility
plus unblocked terrain (or clairvoyance), strict range below 1.9, no already visited
or combat-safe candidates, and Swarm-1's friendly filter. The launcher is eligible
for ordinary Swarm. Cumulative range is checked before drawing the next attack;
the reference's ten visited slots permit eleven total attacks. Empty-coordinate
fire retains ordinary terrain handling. AMS is bypassed for both ammunition modes.

Focused acceptance includes all 49 supported launcher/target pairs with restart
replay, initial and secondary misses, capped secondary clusters, mixed-anatomy
retargeting, visibility and immunity filtering, friend-or-foe behavior, self-targets,
installed Mech/vehicle AMS, exact range and visited-slot boundaries, native/Lua
multi-target firing and callback rollback, and catalogue ammunition compatibility.
Full suite and lint evidence will be recorded once the current run finishes.
This milestone does not establish completion of the remaining scoped integration.


Swarm validation: all 1,966 tests pass in `target/swarm-full-final.log`;
all-target Clippy with warnings denied passes in `target/swarm-clippy-final.log`.
The refreshed construction audits remain 1,299 Mechs and all 269 supported
vehicles/VTOLs (`target/swarm-template-audit.json` and
`target/swarm-vehicle-audit.json`). The three rejected Mech constructions retain
excluded LAM equipment. The reference tree is unchanged.

Next verified cockpit gap: `FIRECLUSTER`, `FIREMINE` and `FIRESMOKE` in
`commands/mech_command_catalog.c:109` share the reference's artillery controls.
Rust `ammunition_mode::toggle_cluster` still reads `constructed_units` directly
after shared admission; it needs the existing shared ammunition-control boundary
for vehicle artillery. Smoke/mine payload storage and arrivals exist, but corresponding
cockpit controls require implementation and native/Lua/restart/rollback checks.
Review `mech_weapon_modes.c:690–717` and its common selector eligibility/priority
before editing. `SIGHT`, `WEAPONSTATUS`, `WEAPONSPECS` and `CRITSTATUS` also remain
candidates for the wider command/alias/grammar audit; a missing literal registry
name alone is not proof that the underlying behavior is absent.


### Cluster and missile special-round controls

The artillery-control audit found a reference help/implementation mismatch:
FIRECLUSTER is artillery-only, but FIRESMOKE/FIREMINE pass special-kind 4 and admit
missile weapons, not artillery. TMISSILE=1 does not overlap IDF=0x40 or DAR=0x80,
so the selector's type/mask expression does not exclude indirect/dead-fire missiles.
Do not implement the misleading help description as an artillery-only control.

Rust now shares cluster admission/storage across Mechs and vehicles and exposes
FIRECLUSTER alongside CLUSTER. Shared FIRESMOKE/FIREMINE native/Lua controls select
typed missile Smoke/Mine supplies, preserve normal toggling, and reject artillery,
rockets and one-shot weapons. Literal Smoke/Mine flags are accepted for missiles;
Artemis/Mine and Narc/Smoke retain their distinct guidance meanings. Missile Mine
rounds bypass AMS and both modes retain ordinary missile damage. Authored artillery
payloads continue to generate smoke/mine effects only on arrival.

Focused tests cover all seven chassis, mode/control publication and callback
rollback, saved selections, matching cluster launch supplies and queued arrival
replay, conflicting artillery modes, invalid pilot/index/recycle/power, and missile
Smoke/Mine damage and AMS behavior against Mech and vehicle defenses. Full-suite
and lint evidence will be recorded when the current verification finishes.

Next concrete interface gap: SIGHT in `sensors/mech_scan_view.c:100` parses one
weapon number and forwards the ordinary firing target arguments with `sight=true`.
Audit its common checks, selected/explicit/coordinate/observer targets and
mechanical exceptions in `combat/mech_fire_command.c:446` before implementation.
It preserves HIDE and pending hiding events, but still rejects a unit acting as
an observer. `mech_fire_preparation.c:84–104` skips ammunition checking yet still
rolls gatling preparation. `mech_fire_resolution.c:70–80` invokes
`weapon_fire_roll` before displaying the preview; that function at
`mech_fire_preparation.c:138` consumes normal or dead-fire/close-ELRM attack dice.
Existing Rust `aim_modifiers` clones the random stream, so it is not sufficient
by itself to establish SIGHT parity. Reuse shared target/aim rules with an explicit
preview action policy; do not duplicate firing calculations between chassis or
silently assume the reference preview has no random-state effect. Check artillery,
C3 feedback, partial-cover display and saved random-stream replay as part of delivery.


Special-round validation: all 1,970 tests pass in `target/special-rounds-full-final.log`;
all-target Clippy with warnings denied passes in
`target/special-rounds-clippy-verified.log`. Four new tests cover shared controls,
cluster firing/arrival replay, missile special-round damage/AMS policy, and atomic
control rejection across the supported chassis. The reference tree is unchanged.

The first full run passed 1,969 tests and exposed an existing unseeded-neighbor
outcome in `landing_in_existing_ice_precedes_the_final_upward_breakout`. An isolated
repeat reproduced it on attempt 15: the neighbor's fall breached its reactor;
blast ignition followed by water extinguishing added a valid Steam/Smoke overlay.
The landing-order test now pins that neighbor's dice and keeps its strict terrain,
posture, feedback and saved-state assertions. All 105 surface tests and 40 repeated
landing-order cases pass (`target/special-rounds-surfaces-final.log` and
`target/special-rounds-ice-seeded-repeat.log`). No production surface or reactor
behavior was changed.

### Shared mode admission and vehicle Artemis

All firing/ammunition controls now use one mechanical admission helper across
Mechs and vehicles. After authority and power checks, physical weapon loss
precedes recycle, which precedes the manual ammunition-feed jam. Vehicle
critical failures remain distinct: a main-weapon Disabled failure permits a
mode change when no recycle timer exists, but the change does not clear the
failure or allow firing. Jammed/Shorted failures retain their required timers.
The reference selector checks FAIL_AMMOJAMMED separately from physical slot
loss; FAIL_DESTROYED is a temporary failure, not the physical DISABLED_MODE bit.

Artemis control and controller matching are shared. Same-section links work on
all supported classes; Mechs additionally permit head-to-center-torso links,
and ground vehicles permit rear-to-turret links. VTOL class identity governs
this exception even when its movement is stationary. Controller metadata is
derived from existing template and damage state and exposed in vehicle Lua
inspection without new persisted fields.

Focused regressions cover all supported movement classes plus stationary VTOLs,
controller loss and section destruction, invalid and dangling links, compatible
ammunition expenditure, native/Lua equivalence, restart and callback rollback.
Mode checks cover manual feed jams and recycle precedence across all supported
chassis, plus temporary vehicle failures across firing-mode families.

The manual-feed admission work is extended by weapon degradation below.
SIGHT and the broader cockpit acceptance audit remain open.

Mode-admission validation: all 1,974 tests pass in
`target/mode-admission-full.log`. The Artemis reservation assertion was then
strengthened to inspect the returned launch and guidance mode; both vehicle
Artemis tests pass in `target/mode-admission-artemis-final.log`. All-target
Clippy with warnings denied passes in `target/mode-admission-clippy-final.log`.
Formatting and diff checks pass, and the reference tree remains unchanged.

### Enhanced weapon critical damage

Mech impacts now distinguish degraded weapon slots from destroyed equipment.
The critical table accounts for previous damaged slots and forces destruction
past half the mount's slot count. Damaged slots retain their installed mass and
are excluded from later random critical selection. Explosive weapon-critical
handling continues before ordinary degradation. Explicit equipment destruction
remains available as a separate primitive.

One saved slot record drives accuracy, heat, energy damage, feed locking and
attack-roll failure thresholds. Both launcher adapters feed the shared launch
and packet rules; vehicle launchers supply zero enhanced penalties. Mech,
vehicle and terrain recipients use the same energy damage adjustment before
glancing rounding. Minimum range retains the short-range damage bracket, and
network-assisted aiming recalculates the penalty after choosing its bracket.
Critical loader jams remain separate from manual feed jams. Crystal overloads
and feed explosions destroy the mount and resolve internal damage in the
existing firing transaction. Their notices identify the damaged component.
Damaged AMS slots disable the whole automatic defense capability.

Mode admission is now one helper for all firing and ammunition controls,
including one-shot restrictions and enhanced feed locks. The former ammunition
wrapper and repeated one-shot checks have been removed. Lua inspection exposes
slot damage and launch reports retain the resulting penalty and failure cause.
No repair commands, C bridge, or additional persisted penalty copies were added.

Focused evidence covers critical-table thresholds, saved-state validation,
minimum range, attack-roll reuse, damage-before-glancing order, real critical
impacts, heat and mode controls, complete coordinate firing, callback rollback,
restart, and split mounts. The initial full run identified one fixture that
assumed every critical destroyed a weapon. Its destruction-feedback scenario
now selects a destructive table roll explicitly; its exact feedback and replay
assertions pass unchanged. Degradation has separate regressions.

Complete integration remains unproven. Continue the cockpit/SIGHT audit and
remaining configuration, command and combat acceptance checks. Repeated
split-proxy criticals are covered by the follow-up below.

Enhanced-damage validation: the full run passes all 1,982 tests in
`target/enhanced-full-final.log`. A subsequent one-shot failure check confirmed
that internal explosions consume the built-in charge without drawing external
ammunition. That correction passes all 177 library and focused tests in
`target/enhanced-one-shot-final.log`, including the strengthened real-firing
rollback/restart scenario. All-target Clippy with warnings denied passes in
`target/enhanced-one-shot-clippy.log`. Formatting and diff checks pass; the
reference tree remains unchanged. These results establish the implemented
behavior, not completion of the remaining integration acceptance review.


### Repeated split-weapon criticals

Extension hits apply damage to the linked primary slot while leaving the
extension eligible for another critical. A saved damage record holds a set of
distinct effects: repeated hits can add a different effect, but the same effect
on the same slot contributes only once. Destruction thresholds count damaged
slots, not effects. A destructive table result removes every damaged slot and
one additional undamaged slot in mount order; remaining installed slots on the
broken weapon can still take later criticals.

Replay and database restart tests exercise repeated extension hits on AC/20,
LB20-X, Ultra AC/20 and Clan Ultra AC/20 in both section traversal orders. They
verify accumulated penalties, duplicate-effect suppression, physical slot loss,
post-destruction extension hits and exact dice consumption, including the
additional penetration roll after armor runs out. Heavy PPC is not admitted as
a split fixture: its four-slot installation fails the reference construction
minimum, and it lacks the catalogue split flag. Energy-family table coverage
remains in the ordinary weapon-damage tests.

Validation: all 1,984 tests pass in `target/split-damage-full.log`, and all-target
Clippy with warnings denied passes in `target/split-damage-clippy-final.log`.
Formatting and diff checks pass. Overall integration remains incomplete;
continue with SIGHT and the remaining cockpit/operator acceptance work.

### Gatling preparation and sensor aim ordering

The SIGHT trace identified an ordinary-firing discrepancy: gatling preparation
belongs before random sensor aim, not between aim and the attack roll. Direct
Mech and vehicle attacks now prepare one gatling result on the candidate dice
stream and carry it through launch/expenditure. The shared preparation routine
also serves standalone expenditure and coordinate launch; anatomy adapters supply
live firing mode and ammunition-feed facts. No second gatling roll is drawn.

A native/Lua regression covers all seven supported chassis with Beagle probe aim
and both two-round and full-bin supply. It checks the ordered gatling D6, sensor
D3 and attack 2D6, the supply-limited result, exact saved dice, callback rollback,
empty aborted feedback, and database restart followed by native/Lua agreement.
Existing non-random aim and standalone launch paths retain their draw counts.
SIGHT itself remains an open interface: it needs shared target resolution and
explicit preview admission, including its ammunition bypass and preserved hiding.

Validation: all 1,985 tests pass in `target/gatling-preparation-full.log`.
All-target Clippy with warnings denied passes in
`target/gatling-preparation-clippy.log`; formatting and diff checks pass. The
read-only reference tree remains unchanged. This closes the gatling/sensor draw
ordering discrepancy; it does not establish complete SIGHT or cockpit parity.

### Native and Lua sighting

`SIGHT <weapon> [<id> | <x> <y>]` and `btech.unit.sight` now share a host
transaction for all supported chassis. Conventional recipient selection is
shared with FIRE/TIC dispatch, including observer precedence, automatic coolant
self-selection and occupied versus empty coordinates. Artillery sighting and
launch share the same coordinate/observer preparation. Target safeties and
weapon geometry are shared with live attacks, while the mechanical sighting
policy bypasses recycle, posture, ammunition and feed failures. Destroyed,
defensive-only and disabled vehicle weapons still reject sighting. Crew stun,
manual recovery work, cockpit authority and observer-role restrictions remain.

Successful sighting commits only its dice and cockpit notices. Hiding, pending
hiding, weapons hold, inventory, heat, timers, damage and artillery queues stay
unchanged. Gatling draws uncapped preparation intensity even without ammunition;
random sensor aim follows it, then the ordinary attack roll. Dead-fire missiles
and extended LRMs below minimum range retain the lowest-two-of-three roll.
Sighting never enters propellant, loader or critical-failure resolution. Reports
include actual target identity/coordinates, aim, BTH or out-of-range state,
partial cover and consumed dice; C3 range sources produce cockpit feedback.

The dedicated tests cover all seven supported chassis, selected/explicit/occupied/
empty/distant targets, empty bins, recycling, hidden/pending cover, hold,
rollback and restart. Additional cases cover authority and destroyed/defensive
weapon rejection, gatling, dead-fire, ELRM, coolant self-selection, artillery,
observer precedence and C3 range feedback. Stinger sight/fire admission now uses
one airborne-target predicate, including airborne VTOLs; a landed VTOL remains
ineligible. The sensor regression checks identical gatling/sensor/attack ordering
for sighting and firing while preserving sighting's uncapped intensity.

Overall integration remains incomplete. Continue the cockpit catalogue and
operator/configuration acceptance review, including WEAPONSTATUS, WEAPONSPECS,
CRITSTATUS and aimed-section targeting; the absence of SIGHT is no longer the
next interface gap.

Sighting validation: all 1,989 tests pass in `target/sight-full.log`. All-target
Clippy with warnings denied passes in `target/sight-final-clippy.log`;
formatting and diff checks pass. The reference tree remains unchanged. Overall
integration remains active and incomplete against the remaining acceptance list.

### Weapon diagnostics and catalogue reports

Native `WEAPONSTATUS` and `WEAPONSPECS` now use shared Rust equipment reports
across the seven supported chassis. Diagnostics retain stable mount numbers,
component failures, damage penalties, preferred ammunition sources and slot
counts. Shutdown, recycling and depleted bins do not constitute component damage;
`weapons` remains the separate readiness report. A conscious, unblinded passenger
may inspect diagnostics on a map. Specifications work for any cockpit occupant,
including before map placement, and retain destroyed installations while listing
each weapon type once in first-installation order. Both commands accept trailing
arguments and reject switches.

`btech.unit.weapon_diagnostics` and `btech.unit.weapon_specifications` expose
structured, detached rows in trusted Lua callbacks. Specifications take extended
range from server configuration; conventional aim, artillery aim and the report
share the catalogue's effective maximum-range calculation. Artillery ranges use
hexes rather than map-sheet counts. No launcher or combat resolver is duplicated.

The report audit also corrected retained component damage on physically destroyed
Mech slots. Damage reconciliation now clears these obsolete records and effects;
intact slots on a broken split mount remain installed. The split-critical test
checks the existing ordered impact/dice sequence plus diagnostic slot counts and
cleared penalties after destruction. Report tests cover all supported chassis,
empty bins, shutdown passengers, mapless specifications, blindness/consciousness,
native/Lua agreement, callback abort, database reload, artillery ranges and vehicle
jams. `CRITSTATUS`, aimed-section targeting and the wider acceptance review remain
open; these two reports do not establish complete cockpit parity.

Runtime weapon-setting overrides were still open at this report milestone;
the shared runtime weapon settings increment below implements those consumers.

Weapon-report validation: all 1,993 tests pass in
`target/weapon-reports-full.log`. All-target Clippy with warnings denied passes in
`target/weapon-reports-clippy.log`; formatting and diff checks pass. The reference
tree remains unchanged. Overall integration remains active and incomplete.

### Critical-slot inspection

Native `CRITSTATUS <section>` and trusted `btech.unit.criticals` now share a
read-only slot report for all supported chassis. Section aliases use existing
anatomy parsers; every physical slot appears, including six-slot Mech heads and
legs, twelve-slot torsos/arms and vehicle sections. Quads use their own leg names.
The native command requires a conscious, unblinded assigned pilot, accepts the
first section argument, rejects switches and works while shut down or off-map.
Sections without original structure reject inspection; destroyed sections remain
inspectable. Native output pairs numbered columns; Lua uses zero-based slots.

One generic inventory formatter resolves mounts, bins and systems. Chassis
adapters supply section layouts and live slot state. Reports distinguish actual
destruction, intact slots on broken mounts, flood disabling and component damage.
Split proxies name their linked weapon and preserve structural-placeholder display
rules. Empty slots stay empty even after section loss. Ammunition labels include
typed bin modes and installed capacities; unavailable bins suppress quantities in
native output. One-shot/spent/rear flags, Artemis control-slot labels, actuator
names, double sinks, engine technologies and small cockpits use current equipment
facts. Manufacturer labels honor the parts setting. Mixed engine display
precedence shares slot counting with construction without changing mass or combat
classification.

Tests cover every supported section through native commands, detached Lua rows,
callback abort and database restart; passenger, consciousness, blindness and
syntax rejection; mapless shutdown access; quad aliases; damage, flood and section
loss; destroyed vehicle ammunition; one-shot/Artemis/half-ton labels; parts on/off;
Clan equipment; and mixed engine names. The existing split-impact replay test also
checks parent/proxy slot reports through degradation and destruction. Aimed-section
targeting, runtime weapon-setting overrides and the broader acceptance audit remain
open. `CRITSTATUS` is no longer an absent cockpit interface.

Critical-slot validation: all 1,998 tests pass in `target/critstatus-full.log`.
All-target Clippy with warnings denied passes in `target/critstatus-clippy.log`;
formatting and diff checks pass. The read-only reference tree remains unchanged.
Overall integration remains active and incomplete against the remaining acceptance
list. Continue with aimed-section targeting and the operator/configuration audit.


## Anatomical targeting implementation and acceptance

`TARGET <section>` and `TARGET -` now use one control path for all supported
shooters. `btech.unit.target` invokes the same transaction; `aimed_section` returns
a detached saved preference. Selection uses the current target's anatomy and
requires a running unit with its conscious assigned pilot. The preference survives
lock changes, lock loss and shutdown. Ground vehicles reject rotor selections;
VTOL anatomy permits turret selection even when a particular hull has no turret.
Saved ground-vehicle/rotor combinations fail validation.

Conventional aim and SIGHT share the head penalty and targeting-computer terms.
Head aim against a Mech costs seven when immobile and twenty-five when mobile,
overriding computer assistance. Other eligible computer-directed fire changes
its usual minus-one term to plus-three against mobile targets. Both firing
chassis call one anatomical launch policy before defenses, then reuse one
per-packet selection policy before their existing material damage adapters.
Directed vehicle hits preserve the rear diagnostic roll. An absent anatomical
section discards damage after entry rolls rather than aborting the attack;
explicit public damage requests still require an available section.

`tests/btech_aimed_target.rs` exercises every supported firing chassis against
Mech, quad, ground-vehicle and VTOL targets. Tests compare native/Lua controls,
selection persistence, callback rollback, detached reads, authority, head aim,
computer modifiers, actual selected hit sections, exact target dice, saved reload
and absent-turret damage. SIGHT and targeting share test construction helpers.
The fallback comparison advances only the immobile preparation roll and compares
ordinary routing with directed attempts at hidden vehicle front armor and VTOL
rotors.

This does not close all targeting acceptance. The following contracts remain
explicitly unverified by these scenarios:

- Initial aim-roll ordering relative to AMS and subsequent Swarm flights is
  covered by the missile admission increment below.
- Live terrain-driven partial cover and multi-packet changes in head eligibility.
  The anatomical policy's covered-leg, hidden-side, failed-computer and mobile-head
  cases are tested directly; complete changing-terrain scenarios remain to be exercised.
- End-to-end vehicle salvos whose earlier packets cause a crash or position change.
  Direct vehicle and Mech packets now share live direction sampling; all supported
  shooter/target pairs are tested across heading and position changes.
- Broader material dice parity beyond the cases tested here. The missing ordinary
  Mech entry roll is addressed in the material-entry increment below.

The wider operator/configuration audit remains open. `SETVRT` and `SETWBV`
were identified here as requiring one shared value source across reservations,
reports, valuation and saved recycle validation. That work is implemented in
the runtime weapon settings increment below.

Validation for this increment: `target/aimed-full.log` records 2,004 passing tests.
All-target Clippy with warnings denied passes after three field-initializer
shorthand cleanups. The six targeting tests are rerun after that syntactic
cleanup in `target/aimed-final-targeting.log`. The reference tree remains untouched.
The integration goal remains active; the acceptance gaps above are not waived by
this green suite.


## Shared direction and anatomical edge cases

Direct Mech and vehicle salvos now use `hit_direction.rs` to derive each packet's
arc from the current positions and target heading. Fixed impacts keep their
explicit direction. `target_salvo.rs` retains initial admission checks; the vehicle
packet loop samples direction again before selecting anatomy and applying damage.
This removes the vehicle-only cached direction without duplicating range or
heading logic. A component test exercises all 49 supported shooter/target chassis
pairs, rotations and movement between geometry samples. It does not substitute for
an end-to-end crash-during-salvo scenario; most inspected vehicle critical effects
schedule descent rather than rotating the unit immediately.

The anatomical policy tests cover immobile numeric remapping between Mech and
vehicle selections, hidden sides, covered legs, quad front-leg slot behavior,
failed computer dice and the restriction on mobile head guidance. Successful
immobile selection consumes no further computer die, including when its location
is hidden; a computer attempt consumes its die before exposure/head admission.

The expanded native/Lua integration fixture can supply an injected weapon with
an explicit ammunition type while preserving existing unsupplied SIGHT cases.
For every firing chassis, tests compare directed and ordinary shots against
Mech, ground vehicle and VTOL recipients after advancing only the initial target
preparation roll. The comparison covers Ultra bursts, LBX clusters, SRM hits and
misses, flamer heat and coolant. Full result and recipient state comparisons prove
that the unused preference neither redirects these packets nor adds another roll.

Explicit attacks against a different target class retain the saved selection.
Integration checks cover both ordinary and computer-equipped firing chassis:
immobile success maps the saved ground rear slot to a Mech right torso, while a
mobile target of another class uses ordinary locations even with a working
computer. SIGHT omits the anatomical suffix for the mismatched target class.
The earlier aim preparation and direction acceptance list above is updated to
retain only gaps not established by these checks. Overall integration remains
incomplete.

Validation: all 2,009 tests pass in `target/aimed-edges-full.log`. All-target
Clippy with warnings denied passes in `target/aimed-edges-clippy.log`; formatting
and diff checks pass. The focused integration run passes eight targeting tests,
and the component checks cover anatomical policy and shared direction sampling.
The `btmux-khi` reference tree remains unchanged. This completes the implementation
increment, not the full integration acceptance audit.

## Missile admission, aimed preparation and Swarm continuation

Mech and vehicle firing now share a target-side missile admission policy in
`target_hit.rs`. The reference rejects missile rolls below the base target
number before entering anti-missile defense or target effects. Near-miss launch
feedback can use a lower threshold, but it does not authorize AMS expenditure,
pod attachment, damage or an initial Swarm flight. Missile cluster glancing uses
the base target number in either enabled glancing mode. Beacon pods and intact
Streak guidance retain their special handling. Swarm follow-up rolls also retain
the original base target number rather than the launch feedback threshold.

This corrects earlier Rust tests and descriptions that expected AMS expenditure
on missed initial attacks. The private AMS entry point accepts admitted attacks
only. Both firing paths preserve initial immobile aimed preparation before that
admission decision, including the unused preparation roll on a missile miss.
Launch classification remains separately visible in the vehicle launch report;
it does not by itself imply material effects.

Integration comparisons exercise all 49 supported firing/defending chassis
pairs with active defenses, hits and misses, exact dice progression, native/Lua
agreement, callback rollback and saved reload. Three-target Swarm and Swarm1
flights compare aimed and ordinary attacks after advancing only the first
target's preparation dice. Near-miss continuation cases verify that a missed
intermediate target passes the remaining missiles to the next recipient without
lowering the target number. Additional native/Lua boundary checks cover normal,
Swarm and Swarm1 ammunition at base minus one, base and base plus one.

The broader integration acceptance gaps remain open, including ordinary Mech
material dice parity and changing-terrain aimed fire; runtime weapon settings
are addressed below.

Validation: all 2,014 tests pass in `target/missile-admission-full.log`.
All-target Clippy with warnings denied passes in
`target/missile-admission-clippy.log`. Formatting, diff checks and the mirrored
Lua declarations agree. The `btmux-khi` reference tree remains unchanged.

## Mech material entry dice

Ordinary biped and quad damage now consumes the diagnostic 2d6 roll at every
material entry, before immunity, crew effects and equipment checks. Damage
transfers and nested ammunition explosions enter separately; progressing from
armor to internal damage within one section does not add another entry.
The same path serves directed hits, conventional salvos, physical attacks, falls
and internal weapon failures. Combat-safe entries retain their existing roll.

`tests/btech_material_entry.rs` checks exact target dice for zero damage, absorbed
armor hits, internal damage, overflow and routing through destroyed sections on
both Mech anatomies, with saved replay. Existing critical, searchlight, split
weapon, fall, collision and reactor scenarios account for the restored entry
ordering. The duel exercises pending crew recovery after restart and cockpit
assignment before testing wreck controls. Charge experience checks reconcile
channel award amounts with actual skill experience, including additional balance
checks caused by damage. This increment does not claim that every remaining
material-damage interaction has completed the wider parity audit.

The surface fixtures now seed both material and recovery streams. The ordinary
ice-entry scenario explicitly disables reactor explosions and reapplies that
runtime policy at each reload; dedicated fracture-cascade coverage retains
reactor blasts and verifies interruption of neighboring falls. This removes
random aftermath from assertions about ordinary ice terrain and observer notices.

Validation covers all 2,015 tests, including every one of the 224 integration
targets. `target/material-entry-final.log` contains the passing groups before
the intermittent surface fixture failure; after correcting that fixture,
`target/material-entry-surfaces.log` passes all 105 surface tests and
`target/material-entry-tail.log` passes the remaining 766 tests. The combined
target audit is recorded in `target/material-entry-verification.txt`.
All-target Clippy with warnings denied passes in `target/material-entry-clippy.log`.
Formatting and diff checks pass, and the reference tree remains unchanged.
The overall integration goal remains active.

## Shared runtime weapon settings

One transactional `BattleWeaponSettings` source supplies recycle times and Battle
Values to Mech and vehicle firing, AMS activation, weapon specifications, unit
valuation and the Battle Value experience formula. The immutable catalogue keeps
the initial values; overrides reset when the database is reloaded, matching the
reference runtime lifetime. Standalone catalogue valuation and formula helpers
remain available for asset calculations, while world-aware consumers use runtime
values.

Wizard commands `@btech setvrt <weapon> <seconds>` and
`@btech setwbv <weapon> <value>` accept canonical weapon names. Recycle values
range from 1 through 127; Battle Value ranges from zero through signed-32-bit
maximum. `@btech weapon-settings <weapon>` reports both effective values. Lua
exposes detached inspection and authorized setters under `btech.weapon`.
Setters validate before mutation and participate in native/Lua rollback.

New activations reserve the current recycle value. Changing a setting does not
rewrite existing timers, and restart preserves those countdowns while restoring
catalogue settings. Both unit validators admit positive ordinary countdowns through the
shared maximum, so a 127-second timer remains valid after a reset to a shorter
default. Zero and out-of-range timers remain invalid. Vehicle critical-failure
recovery retains its separate 120-second bound.

Integration tests cover all seven firing chassis and all 49 supported
firing/defending pairs, native/Lua agreement, permissions, boundary values,
callback rollback, AMS interception, detached reads, restored defaults and
countdown replay. Offensive/defensive valuation and actual experience awards
exercise the overrides, including zero weapon BV's defined minimum award.
The broader operator and gameplay parity audit remains active.

The runtime-settings implementation passed all 2,019 tests in
`target/weapon-settings-final.log` and all-target Clippy with warnings denied in
`target/weapon-settings-clippy.log`.

### Manufacturer-qualified weapon controls

Operator selection now accepts exact manufacturer-qualified names such as
`Magna.IS.MediumLaser` in addition to canonical names. Native commands and Lua
inspection/setters share the resolver, and cockpit labels share the same
manufacturer table. A qualified name updates the weapon identity globally,
independent of chassis or installed manufacturer. Invalid weapon/manufacturer
combinations and qualified Clan names are rejected before mutation. Template
weapon identifiers retain their canonical grammar.

The reference `PART_MATCH_VERY_LONG` lookup used by SETVRT/SETWBV is exact and
case-insensitive; wildcard matching belongs to its separate LONG lookup. This
increment therefore does not add wildcard selection. Reference observations:
`unit/mech_partnames.c`, `unit/template_format.c`, `character/failures.c`.

Manufacturer lookup validation passes 25 tests across runtime settings, weapon
reports, critical reports, equipment names, catalogue and help. Logs are
`target/weapon-manufacturer-tests.log` and
`target/weapon-manufacturer-surface.log`. All-target Clippy with warnings denied
passes in `target/weapon-manufacturer-clippy.log`; formatting, diff checks and
Lua declaration mirrors also pass. This focused check follows the complete
2,019-test runtime-settings run above; it is not a new full-suite run.

### Remaining parts-cargo integration

The supported command audit identifies MANIFEST, STORES, LOADCARGO and
UNLOADCARGO, together with the Wizard inventory edits, as unfinished. They are
parts inventory operations, distinct from deferred unit/container loading.
`economy/econ_cmds.c` supplies the observable access, hangar, stationary-load,
transfer and speed-correction behavior. The inventory storage increment below now supplies stock records and Wizard
corrections; the named cargo commands remain unfinished. Mech construction currently rejects nonzero
cargo capacity; vehicle mass accounts for cargo construction space without a
runtime contents model.

Completing this area requires one shared typed parts inventory for rooms and
units, persistent atomic transfers, configured access and bay checks, common
cargo-mass effects, native/Lua operations and saved replay across the supported
chassis. Repair workflows and excluded unit types remain outside this work.

## Shared loose-parts inventory

The existing `btech_economy_parts` table is now loaded into one object-owned,
ordered stock model rather than being left outside Rust world snapshots.
Rooms, Mechs and vehicles use the same entries, setters and persistence path.
Entries preserve the game directory's part and manufacturer identifiers;
quantities are positive signed-32-bit values, and zero in a stock correction
removes an entry. Snapshots reject negative identifiers, invalid manufacturer
values, duplicate or unordered entries, empty stored manifests and missing owners.
Deleting an owner removes its inventory through existing maintenance cleanup.

Wizard `@btech inventory` and `@btech inventory-set` operations expose inspection
and exact stock correction. Lua `btech.inventory.read` returns detached rows;
`btech.inventory.set` checks Wizard authority and participates in the callback
transaction. Selective writes retain unrelated database columns. This is loose
stock, not installed equipment: edits do not change critical slots.

This increment establishes shared inventory ownership and persistence. Named
part selection, cargo mass and speed correction, load/unload admission, transfer
points and cockpit manifests remain required before cargo gameplay is complete.
Unit/container loading and the user-excluded systems remain deferred.

Further cargo observations for the next implementation step:

- The database part identifiers are independent of the Rust weapon enum order.
  Weapon IDs start at 1, ammunition at 193, specials at 394 and commodities at
  512 (`unit/equipment_types.h`). Resolve these through catalogue identities;
  never infer them from Rust enum discriminants.
- `economy/econ_cmds.c` transfers the lesser of requested and available stock for
  each matched part. The inspected load/unload path has no hard cargo-capacity
  rejection; it checks cargo technology, ownership/location and the applicable
  movement/hangar rules, then corrects speed. Construction cargo space must not
  be turned into an invented gameplay limit.
- `movement/mech_move.c` applies cargo mass separately from tow discounts: ordinary
  Mechs use twice the cargo mass, CargoTech Mechs use its full mass, ordinary
  vehicles use full mass and CargoTech vehicles use half. The same shared load
  projection should own these effects.
- `btech_map_cargo_configuration` already stores optional transfer coordinates
  and a reveal-hint flag. Its reference setter validates map coordinates
  (`core/configuration.c`); this table remains outside Rust runtime state.

Inventory validation: the full suite passed 2,022 tests in
`target/inventory-full.log`. The expanded four-test inventory target then passed
in `target/inventory-expanded-tests.log`, including two additional malformed
snapshot/owner-cleanup and database-rollback tests. Deleted owners are reusable
Garbage slots, so the final domain check rejects inventory access on those slots;
72 affected database, maintenance and help tests pass in
`target/inventory-surface-tests.log` after that refinement. These runs cover all
2,024 current tests; the 2,022-test full run predates the final live-owner check.
All-target Clippy with warnings denied passes in `target/inventory-clippy.log`.
Formatting, diff checks and the Lua declaration mirror pass; the reference tree
remains unchanged. Cargo gameplay and the broader integration remain active.

## Named stock and shared cargo mass

The stock catalogue now describes 575 stable database identifiers. The 141
simulated weapon identities declare their inventory IDs in the existing weapon
catalogue; stock lookup reuses their names and masses. Other weapon stock,
components, commodities and bombs have declarative physical metadata. Possessing
these items does not enable excluded combat systems or unit types. Ammunition
identities retain their database offset and one-ton stock mass.

Wizard inventory edits accept exact part names as well as identifiers; listings
show names. Lua adds detached `btech.inventory.part(name_or_id)`, physical
`btech.inventory.mass(object)` and transactional `btech.inventory.set_named`.
Names ignore ASCII case. Duplicate catalogue names (`Steel` and `CASE-II`) are
rejected as ambiguous; numeric edits select the exact record. Unknown numeric
stock can still be retained on ordinary objects, but live units require known
mass metadata before an edit can commit.

Physical stock mass uses checked integer arithmetic. Loose bomb stock receives
its fourfold mass multiplier. Cargo mass enters the existing shared load
projection separately from towing discounts: ordinary Mechs use twice physical
mass, CargoTech Mechs and ordinary vehicles use full mass, and CargoTech vehicles
use half after summing all stock. Mech construction now admits the CargoTech flag;
nonzero Mech construction cargo space remains unfinished. Zero-mass structural
markers do not activate a propulsion penalty.

Throttle controls, effective speed and heartbeat movement share the loaded
ceiling. The heartbeat's former towing-only clamp now uses a common cargo/tow
predicate across Mechs, ground vehicles and VTOLs. Removing stock restores the
unloaded throttle behavior. Existing tow relationships and their discounts remain
independent of cargo.

The reference component weight table has fewer entries than its name table.
Rust explicitly gives Light BAP stock its half-ton probe mass and gives split
links/hardpoint markers zero physical mass, avoiding undefined table access.
Other defined weight entries retain their actual positional values. Reference
facts were checked against `unit/weapons_catalogue.c`, `unit/equipment_types.h`,
`unit/template_internals.c`, `unit/template_cargo.c`,
`unit/template_cargo_weights.c`, `economy/unit_cost.c`, `economy/econ_cmds.c`, and
`movement/mech_move.c`; no reference files were modified.

Tests cover stable IDs and existing weapon facts, fixed mass examples, all seven
supported movement classes with and without CargoTech, integer rounding,
overload, stock removal, native/Lua agreement, detached inspection, callback
rollback, mixed towing/cargo load and actual heartbeat speed clamping with saved
replay. Cockpit MANIFEST/STORES/LOADCARGO/UNLOADCARGO, bay admission and transfer
points remain required. VTOL fuel-tank stock interactions also require review;
this increment does not claim cargo gameplay is complete.

Cargo command admission observations for follow-up: reference LOADCARGO requires
started power, the assigned conscious pilot, CargoTech, zero current speed,
an out-of-character hangar and any configured transfer point. UNLOADCARGO uses
the no-started-power common check and still requires CargoTech, pilot ownership
and matching unit/map object locations; its path does not impose the load-only
speed, out-of-character or transfer-point checks. STORES requires started power
and the hangar/point checks but not the pilot-only flag. MANIFEST checks the
cargo-command configuration and lists the actor's current location. Keep these
differences explicit above shared inventory transfer logic rather than applying
one stricter gate to every operation (`economy/econ_cmds.c`,
`unit/mech_status_types.h`).

Validation for named stock and mass: the full suite passes all 2,029 tests in
`target/stock-mass-full.log`. The final construction preflight checks also pass
all ten tests in `target/stock-mass-construction-tests.log`: constructing a
stocked object validates known mass before mutating either unit registry, and
valid stock is retained. The earlier focused catalogue/inventory/towing run
passes 43 tests in `target/stock-mass-tests.log`; zero-mass stock checks pass in
`target/stock-mass-final-focused.log`. All-target Clippy with warnings denied
passes in `target/stock-mass-clippy.log`. Formatting, diff checks and the mirrored
Lua declarations pass. The reference tree remains unchanged, and the overall
integration goal remains active.

## Saved cargo transfer points

Maps now own an optional `BattleCargoTransferPoint` restored from the existing
`btech_map_cargo_configuration` table. Coordinates are validated against the
owning map. Missing owners, out-of-range coordinates and malformed hint flags
are rejected during load. Selective updates retain unrelated table columns;
clearing a point deletes its row, map cleanup removes it and same-size terrain
reload retains the setting.

Wizard `@btech cargo-point <map> [x y [hide|reveal]|clear]` inspects or configures
the point. Lua offers detached `btech.map.cargo_point(map)` and authorized,
transactional `btech.map.set_cargo_point(actor,map,point_or_nil)`. Omitted
`reveal_hint` defaults to false. Shared location checks reveal coordinates only
when configured to do so; ordinary failure messages do not disclose them.

Tests cover bounds, permissions, hidden/revealed errors, native/Lua agreement,
late callback rollback, detached reads, nil after clearing, loading authored
rows, selective writes, terrain reload, clearing, owner cleanup and malformed
database records. This supplies the common bay-location check; cockpit cargo
commands and atomic inventory transfers remain unfinished.

Further inspection of `commands/mech_command_checks.c` refines the earlier
cargo admission notes: reference pilot-only enforcement applies to non-Wizards
on in-character units. Consciousness checks depend on the unit's pilot and
power state; unloading does not use the started/destroyed guard. The next cargo
command implementation must account for these exceptions rather than blindly
reuse the stricter currently-assigned-pilot helper used by combat commands.

Reference cargo quantities clamp requests to 50,000 before taking the lesser of
requested and available stock (`core/btconfig.h`, `economy/econ_cmds.c`). The
existing Wizard inventory setter is an exact stock correction, distinct from
those future transfer/add/remove operations. Transfers must preflight both
inventories and receiving-unit mass before publishing either change, then
reconcile the loaded throttle without advancing simulation time.

Cargo transfer-point validation passes all 2,033 tests in
`target/cargo-bay-full.log`. Final saved-map inspection and detached-point
coverage passes all eight tests in `target/cargo-bay-final-focused.log`.
All-target Clippy with warnings denied passes in `target/cargo-bay-clippy.log`.
Formatting, diff checks and the Lua declaration mirror pass. The reference tree
remains unchanged. Cargo command admission and atomic stock transfers are still
required; the overall integration goal remains active.

## Shared cockpit cargo commands

Native `manifest`, `stores`, `loadcargo` and `unloadcargo` now call the same
stock-report and transfer operations exposed by `btech.cargo`. Transfers prepare
all selected stock changes and immediate throttle correction in one candidate
world, then publish together. Mechs, ground vehicles and VTOLs share matching,
quantity limits, inventory validation and transfer logic. No unit-inside-unit
loading is introduced.

Admission preserves operation differences: loading requires CargoTech, running
power, no actual movement, an out-of-character map and its configured loading
point. Unloading omits those load-only power, movement and map restrictions.
Stores omits pilot-only and CargoTech checks; manifest inspects the actor's
current holder. Global cargo access applies even to Wizards. In-character
pilot-only enforcement permits Wizard override and does not restrict ordinary
out-of-character passengers. Blindness and pilot consciousness are checked.

Patterns support case-insensitive names, numeric part IDs, known weapon
manufacturer names, `*`, `?` and escaped literals. A positive request is capped
at 50,000 per matched row and available stock. Unknown stock mass or destination
overflow rejects the entire transfer. This is not a completed compatibility
claim for reference short abbreviations and name-search priority; those,
economy-channel messages, VTOL fuel-tank stock effects and nonzero Mech
construction cargo space still require implementation or acceptance review.

`tests/btech_cargo.rs` covers all seven supported chassis, restart, stock mass,
quantity caps, multi-row rollback, operation-specific authority and map gates,
configuration denial, native/Lua agreement, detached reports, late Lua rollback,
manufacturer matching, moving-load rejection, moving unload and immediate
pending-throttle reduction. Player help is in `game/help/cargo.md`; Lua type
annotations and command-access catalog coverage include all four operations.

Validation for shared cargo commands: all 2,039 tests are verified across
`target/cargo-full.log` (1,780 passing tests before the command-list expectation
failure) and `target/cargo-remaining.log` (259 passing tests after updating the
command list, covering the failed target and all subsequent integration targets).
The four native commands are recorded in both catalog expectations and their
access-policy coverage. Documentation tests pass in `target/cargo-doc.log`;
all-target Clippy with warnings denied passes in `target/cargo-clippy.log`.
Formatting, diff checks and mirrored Lua declarations pass. The read-only
reference tree remains unchanged.

## Cargo abbreviation and exact-name priority

Cargo transfer selection now uses immutable catalogue indexes for exact short
abbreviations and exact full names, before considering wildcard stock matches.
Indexes are shared across chassis and constructed independently of available
stock. Exact collisions select the lowest manufacturer number, then lowest part
ID. This prevents an empty exact selection from falling through to a different
stocked identity. For example, `Ma.ML` identifies Martell even when only Magna is
stocked; `Magna.IS.MediumLaser` disambiguates Magna explicitly. Duplicate `Steel`
identities are distinguishable by numeric ID. Numeric and wildcard requests
retain explicit multi-row selection; manifest/stores retain wildcard reporting.

The compact spelling keeps capitals, digits and underscores, preserving short
names of at most four characters unless they contain a slash. Weapon and
ammunition abbreviations omit the Inner Sphere prefix and retain Clan identity.
Manufacturer names reuse the installed-weapon catalogue helper. Qualified short
wildcard labels such as `Magna.MediumLas?r` are accepted alongside canonical
labels. Name lookup and wildcard matching live in one stock-selection module;
transfers retain their existing shared atomic implementation.

Tests exercise abbreviation facts, collisions, empty exact stock, manufacturer
identity, duplicate commodity names, explicit numeric selection, native/Lua
agreement and restart on all seven chassis. This closes the common transfer
abbreviation/priority gap for the represented stock and manufacturer catalogue.
The rewrite still retains useful unqualified wildcard and numeric selectors;
this does not assert identical behavior for every reference name-registry quirk
or manufacturer metadata for non-simulated weapon stock. Economy-channel messages,
VTOL fuel-tank stock interactions and nonzero Mech construction cargo space
remain required cargo work.

Validation: 19 cargo, stock-mass and help tests pass in
`target/cargo-selection-focused.log`, plus the abbreviation unit test in
`target/cargo-selection-unit.log`. After the Clippy style correction, the unit
test and all seven cargo tests pass again (`target/cargo-selection-final.log`).
All-target Clippy with warnings denied, formatting, diff checks and mirrored Lua
declarations pass. The full suite was not repeated for this isolated selector
change; the preceding integration baseline verified 2,039 tests. The reference
tree remains unchanged.

## Shared cargo-space construction

Mech construction now admits nonnegative authored `Cargo_Space`, using the same
mass calculation as vehicles. The common helper retains floating-point conversion
and truncation: capacity divided by 500 normally, 100 with CargoTech, or 1000
with Carrier_Tech, then scaled to 1/1024 tons. Carrier_Tech takes precedence.
The Mech mass report exposes `cargo` alongside the vehicle report. This is
installation mass and remains separate from inventory mass and its discounts.

Nonzero installations also activate shared load-based propulsion accounting
while empty. An overweight empty cargo construction can therefore lose speed
or become immobile without requiring a stock item to activate the limit.
Construction rejects malformed capacities, component overflow and total mass
overflow before publishing state. Saved Mech validation checks nonzero cargo
mass again; no new persistence table or migration is needed because the authored
capacity remains part of the saved construction definition.

Tests cover fractional rounding, both technology flags and precedence, all seven
supported chassis, loose-stock separation, Lua mass reports, saved restart,
invalid values, component/total overflow, section loss and empty overloading.
Battle armor capacity and unit-inside-unit loading remain excluded. This closes
the nonzero Mech cargo-space construction gap; economy-channel messages and VTOL
fuel-tank interactions remain cargo work.

Validation: all 2,045 tests are verified across `target/cargo-space-full.log`
(548 passing tests before an obsolete positive-capacity rejection expectation)
and `target/cargo-space-remaining.log` (1,497 passing tests after updating that
expectation, including the failed target and every subsequent integration target).
The earlier motion-focused run passes 52 tests. Documentation tests pass in
`target/cargo-space-doc.log`; all-target Clippy with warnings denied passes in
`target/cargo-space-clippy.log`. Formatting, diff checks and mirrored Lua type
annotations pass. The read-only reference tree remains unchanged.

## Transactional cockpit economy diagnostics

Native and Lua cargo transfers now share `transfer_cargo_action`, which wraps
stock changes, throttle reconciliation and channel publication in one world and
effects checkpoint. Each moved row produces its unit-side record followed by
its map-side record on `MechEconInfo`, using the actual transferred count. Load
records add to the unit and remove from the map; unload reverses the signs.
The low-level world-only transfer operation remains useful for simulation,
while host command and Lua adapters use the publishing action.

The existing channel service owns listeners, message counts, history and missing
channel behavior. Cargo commands do not create channels. A channel failure,
including overflow on the second record after the first was staged, restores
stock, throttle, channel state and notifications. Lua late errors and protected
calls preserve the same rollback guarantees. Reports do not emit transfer logs.

Tests cover ordered native/Lua records, actual quantities, saved stock/history,
late callback rollback and second-record overflow for all seven supported
chassis. Existing no-channel tests continue to exercise successful transfers
without diagnostics. This delivers cockpit-transfer economy messages; operator
inventory corrections and other economy interfaces still need their separate
reference audit. VTOL fuel-tank stock interactions remain required cargo work.

Validation: all 45 cargo, inventory, radio-diagnostic and help tests pass in
`target/cargo-economy-focused.log`, including all nine cargo tests. All-target
Clippy with warnings denied passes in `target/cargo-economy-clippy.log`.
Formatting, diff checks and mirrored Lua declarations pass. The preceding full
integration baseline verified 2,045 tests; this isolated adapter/channel addition
was validated with the affected tests rather than another full-suite run.
The read-only reference tree remains unchanged.

## VTOL auxiliary fuel-tank cargo

VTOL fuel inspection now derives maximum capacity from the original template
capacity plus 2,000 per loose `Fuel_Tank`, summing all manufacturer stock rows in
wide checked arithmetic. Loading tanks does not create fuel. Unloading tanks
reduces capacity without discarding fuel already aboard; saved fuel therefore
retains a bounded 32-bit remaining counter independently of current capacity.
Original capacity is still validated against the template, and the exhausted
sentinel remains -1. No inventory-dependent maximum is persisted or cached.

Fuel above the original capacity contributes additional cargo mass before the
existing chassis cargo adjustment. The shared load predicate includes that
surplus even after all tank stock is removed. Fuel consumption reduces the
surplus, and correcting fuel back to original capacity restores unloaded limits
when no other load remains. Tank stock retains its ordinary mass on all chassis;
ground vehicles and Mechs do not gain a flight-fuel subsystem.

Wizard `@btech fuel <unit> [amount]` inspects or corrects remaining fuel, bounded
by current capacity and the 32-bit counter. Lua uses `btech.unit.fuel(unit)` and
transactional `btech.unit.set_fuel(actor,unit,amount)`. Cockpit status and
`btech.unit.state(unit).fuel` use the same detached projection. Fuel corrections
reconcile load without repairing rotor damage or cancelling a fall.

Tests cover capacity across brands, empty tank loading, native/Lua correction
parity, cockpit and Lua reports, surplus mass and unloading, restart, consumption,
large stock/counter bounds, authority, invalid amounts, detached reads and late
callback rollback. Installed Fuel_Tank criticals remain unsupported construction
work; this increment handles loose tank stock. Operator inventory-correction
logging and other economy interfaces also retain their separate audit.

Validation: all 76 cargo, stock-mass, VTOL fuel/flight/control/crash/speed, status,
vehicle-administration and help tests pass in `target/fuel-cargo-acceptance.log`.
The final surplus-only speed/restoration checks pass with all 12 cargo tests in
`target/fuel-cargo-final.log`. All-target Clippy with warnings denied passes in
`target/fuel-cargo-clippy.log`. Formatting, diff checks and mirrored Lua
annotations pass. This increment used affected-suite validation; the preceding
full integration baseline remains 2,045 verified tests. The reference tree was
not modified.

## Installed VTOL fuel tanks

`Fuel_Tank` is now a typed system critical. VTOL fuel capacity combines installed
tank slots and loose tank inventory in the existing fuel projection. The report
exposes `installed_tanks` separately from carried `auxiliary_tanks`, and Wizard
fuel inspection reports both. Original starting fuel remains unchanged.

Installed tank weight enters the shared cargo load using the stock catalogue's
mass, including the usual CargoTech adjustment. It is not added to chassis
material mass. Installed slots retain their capacity and cargo contribution after
critical or hull-section damage, matching the reference cargo recalculation.
Ground vehicles can retain tank equipment without gaining a flight-fuel subsystem
or adding duplicated material mass. The same typed equipment parser and ordinary
slot reporting are used across construction paths.

Tests cover mixed installed/carried tanks, ordinary and CargoTech mass adjustment,
original fuel, full-capacity correction, Lua and Wizard inspection, saved restart,
damaged slots/sections and ground-vehicle behavior. No source templates or files
in the read-only reference tree were modified. Operator inventory corrections and
remaining economy interfaces still need their separate integration audit.

Validation: all 89 installed-tank, cargo, mass, loadout, critical-report, VTOL
flight/speed/fuel and help tests pass in `target/installed-tanks-acceptance.log`.
The final Wizard-report assertion passes with all three installed-tank tests in
`target/installed-tanks-final.log`. All-target Clippy with warnings denied passes
in `target/installed-tanks-clippy.log`; formatting, diff checks and mirrored Lua
annotations pass. This increment used affected-suite validation. The preceding
full integration baseline verified 2,045 tests; the reference tree is unchanged.

## Transactional Wizard stock corrections

Wizard `@btech inventory-set`, Lua `btech.inventory.set` and `set_named` now use
one publishing action. It preserves the low-level stock validation, immediately
reconciles load for constructed units, and emits the actual signed quantity
change to an existing `MechEconInfo` channel. Unchanged quantities do not emit a
record. Rooms and other ordinary holders retain stock editing without unit load
requirements. Named and numeric adapters resolve to the same action.

Stock corrections and cockpit transfers now share one economy-message formatter.
A failed correction, throttle calculation, channel publication or enclosing Lua
callback restores inventory, unit motion, channel history and staged effects.
The world-only quantity setter remains the mutation primitive used by prepared
simulation candidates and fixtures.

Tests exercise all seven chassis, movement clamping without a tick, signed delta
records, zero-quantity removal, no-op assignments, native/numeric/named agreement,
late callback rollback, protected Lua publication failure and saved stock/history.
The separate reference add/remove/reset operator commands and other economy
interfaces still require their command-contract audit and integration.

Validation: all 37 cargo, inventory, stock-mass, cargo-space, installed-tank and
help tests pass in `target/inventory-audit-acceptance.log`. All-target Clippy with
warnings denied passes in `target/inventory-audit-clippy.log`; formatting, diff
checks and mirrored Lua annotations pass. This action-level change used affected
suite validation; the preceding full integration baseline remains 2,045 verified
tests. The read-only reference tree remains unchanged.

## Catalogue add/remove/clear stock commands

Wizard `ADDSTUFF`, `REMOVESTUFF` and `CLEARSTUFF` now operate on the actor's
current holder. Lua offers matching `btech.inventory.add`, `remove` and `clear`
operations with an explicit holder. All use one typed stock-change action and
shared catalogue selection, inventory validation, load reconciliation and
transactional economy publication. The native reset spelling is CLEARSTUFF;
`mech_rresetstuff` is the reference implementation's internal name.

Add/remove searches the catalogue even when stock is absent, with exact
abbreviation/full-name priority before wildcard matching. Positive requests cap
at 50,000 per selected entry, and non-GOD Wizards may select at most 20 entries.
Removal floors stored quantities at zero while retaining requested amounts in
operator confirmations and diagnostic records. Positive actuator component
balances follow the reference generic Actuator stock rule. Clearing removes
all stored rows, including unknown imported identifiers, and emits one reset
record even for an empty holder.

A candidate world contains the complete batch before publication. Any invalid
edit, quantity overflow, load failure, diagnostic error or enclosing callback
failure restores the batch and staged messages. Native command registration,
permission fixtures, Lua annotations and Wizard help include all three commands.
Tests cover every supported chassis, native/Lua agreement, catalogue additions,
request caps, manufacturer abbreviations, actuator stock, clamped removal,
reset/restart, unknown-row clearing, the GOD match-limit exception, authority,
late callback errors and multi-row publication failure. The remaining economy
and operator interfaces still need their broader catalogue audit.

Validation: all 54 cargo, inventory, stock-mass, command, access and help tests
pass in `target/stock-commands-acceptance.log`. All-target Clippy with warnings
denied passes in `target/stock-commands-clippy.log`; formatting, diff checks and
mirrored Lua declarations pass. This increment used affected-suite validation;
the preceding full integration baseline verified 2,045 tests. The reference tree
remains unchanged.

## Scripted single-match store adjustments

`btech.inventory.add_stores(actor,object,pattern,count)` implements the separate
scripted store contract. It selects one exact abbreviation/full-name match or
the first wildcard match in long-name order. Positive counts cap at 50,000;
negative counts retain the full signed 32-bit request and floor stored stock at
zero. Zero succeeds before matching, while a nonzero unmatched or empty pattern
returns false. Target, Wizard authority and the less-than-2048-byte pattern bound
are checked before the zero-count success path.

The scripted function and multi-match operator tools now share stock delta
arithmetic and actuator accounting. The single-match diagnostic retains the
signed requested count and canonical part name. Stock, immediate load correction,
channel effects and surrounding Lua work share rollback. The modern Lua entry
point lives with the other inventory operations rather than creating a second
parts subsystem.

Tests cover first-match manufacturer ordering on all seven chassis, positive
caps, uncapped negative counts including the signed minimum, zero/no-match/empty
patterns, target/authority/name bounds, channel failure under pcall, late callback
rollback and restart. This closes the represented-catalogue scripted store gap;
the broader economy/operator catalogue audit remains incomplete.

Validation: all 34 cargo, inventory, stock-mass and help tests pass in
`target/add-stores-acceptance.log`. All-target Clippy with warnings denied passes
in `target/add-stores-clippy.log`; formatting, diff checks and mirrored Lua
annotations pass. This increment used affected-suite validation. The preceding
full integration baseline remains 2,045 verified tests, and the reference tree
remains unchanged.


## Cockpit catalogue names: channel listing and sword attacks

The reference cockpit catalogue exposes `LISTCHANNELS` and `CHOP`; Rust had the
underlying behavior under `listfreqs` and `sword`. Both reference names now use
those same handlers. Radio listing shares cockpit admission and rendering across
all supported chassis. `chop` shares sword targeting, damage, notices and recovery;
its usage and rejected-switch messages identify the invoked command. The existing
names remain available. Help and native access expectations include both names.

The radio integration test now rotates all seven supported chassis through sender,
relay and receiver roles, checks identical listing output, and retains transmission,
rollback and restart checks. Sword hit/miss scenarios compare `chop` against the
native/Lua sword result, including saved state and emitted messages. Switch rejection
preserves unit state. Validation passes 32 affected tests: 31 in
`target/cockpit-aliases.log` and one selected physical-attack test in
`target/chop-alias.log`. All-target Clippy with warnings denied, formatting and
diff checks pass; the reference tree remains unchanged.

The catalogue comparison identified `DISABLE <weapon>` as a separate supported
behavior, implemented in the next entry. The reference `unit/mech_advanced.c` permits powering down
Gauss weapons after their recharge, rejects other weapon types and destroyed
mounts, and records a persistent temporary-failure state. It is not a unit repair
command. Its firing, explosion, critical-report and restart interactions
require verification against both chassis state models.
`ATMRANGE`/`ATMEXPLOSIVE` also require a weapon-capability audit: ATM stock catalogue
entries alone do not prove simulated weapon or ammunition-mode support. These
findings keep the full integration acceptance audit open.


## Gauss weapon power-down

Native `disable` and Lua `btech.unit.disable(unit, pilot, weapon)` share a Rust
action that powers down Gauss weapons after recharge. The native adapter reuses
weapon controls' existing range/comma selection, partial-rejection ordering,
notifications and transaction boundaries. Duplicate selections succeed without
changing the already recorded state, matching the reference's separate physical
and temporary-failure checks. The action requires a running, mapped unit and the
conscious assigned cockpit pilot through the existing shared control boundary.

Both chassis models save a set of powered-down mount numbers. Shared validation
permits only installed Gauss identities, including mounts subsequently destroyed.
The state does not consume ammunition, remove material, add heat/recovery or roll
dice. It survives ordinary shutdown and restart. Shared mechanical admission
rejects firing; weapon and critical-slot reports identify disabled installations.
Mech and vehicle critical paths still destroy the physical installation but
suppress the Gauss explosion. Transient jams and their recovery timers cannot
restore the deliberately powered-down weapon.

Tests exercise all five supported Gauss variants across seven chassis, native/Lua
state equality, late-callback rollback, exact non-power state preservation,
firing rejection, report output, shutdown/restart and persistence. Admission tests
cover wrong actors, non-Gauss weapons, recharge, shutdown, map removal, invalid
indices, mixed native selections and invalid saved identities. Critical tests
verify inert destruction for Mechs and mobile vehicles/VTOLs; stationary vehicles
retain their existing no-effect advanced critical table. The full integration
acceptance audit remains open, including broader command-admission parity.

Validation: `cargo test` passes all 2,062 tests, with no failures, in
`target/gauss-power-full.log`. All-target Clippy with warnings denied passes in
`target/gauss-power-clippy.log`. Formatting, diff checks, the Lua declaration
mirror and the unchanged reference-tree check also pass.


## Clan Streak LRM family

All four Clan Streak LRM sizes now use the shared simulated weapon catalogue,
retaining stock IDs 155–158 and ammunition IDs 347–350. Catalogue data comes from
`unit/weapons_catalogue.c`, `unit/weapons_vrt.h` and the Clan cluster rows in
`unit/mech_build.c`. They retain a six-hex minimum range, 7/14/21 range bands,
one damage per missile, and 15/20/25/30-second recycle. Their reference flags do
not grant indirect fire or hotloading. No per-chassis launcher logic was added.

Shared Streak launch admission preserves ammunition and heat on failed locks
while starting ordinary recycle. Successful locks use the full salvo as
individual one-point hits, including glancing hits. Angel confusion switches to the
matching Clan LRM cluster table through the existing shared packet resolver;
glancing penalties then apply to that conventional table. Native/Lua firing,
missile interception, damage publication and persistence use existing paths.
The duplicate inventory-only weapon rows were removed; the stock catalogue
still contains 575 identities with unchanged masses. The simulated catalogue
now has 145 weapons; snapshot and Lua declarations include the four new names.

Acceptance exercises every launcher size across all seven attacker chassis,
successful and failed locks, native/Lua equality, callback rollback, ammunition,
heat, recycle and restart. AMS checks cover all 49 supported attacker/defender
pairs. A real Angel field is exercised with all four sizes through vehicle
firing; shared packet tests cover all 256 seeded streams, both confusion states
and both glancing states against the Clan tables. Empty dedicated ammunition
is rejected without changing state. ATM delivery is recorded below; MML remains
in the supported backlog.

Validation passes 60 affected tests: 52 in `target/streak-lrm-acceptance.log`,
one shared packet test in `target/streak-lrm-clusters.log`, and seven help tests
in `target/streak-lrm-final.log` (which also reruns the four new integration
tests). All-target Clippy with warnings denied, formatting, diff checks, the
Lua declaration mirror and the unchanged reference-tree check pass. The last
full-suite baseline remains the preceding 2,062-test run; this increment uses
focused acceptance and does not claim completion of the full integration.


## ATM launchers and ammunition markers

Clan ATM 3/6/9/12 now use the shared simulated weapon catalogue. Weapon stock
IDs 65–68, ammunition IDs 257–260 and masses remain unchanged; inventory-only
weapon rows were removed without changing the 575-item stock catalogue. The
simulated weapon catalogue now contains 149 identities. Catalogue snapshot and
Lua weapon annotations include all four new weapons.

`atmrange` / `atmexplosive` and corresponding Lua unit methods share the existing
multi-weapon control, ammunition feed, native notification and rollback paths.
They toggle the typed `ExtendedRange` / `HighExplosive` template and saved-state
markers. The reference controls permit eligible indirect launchers beyond ATMs;
rockets, artillery, ordinary SRMs, Streaks and energy weapons are not admitted.
A selected mode requires its matching ammunition bin. Repeating a toggle returns
to normal; selecting the other mode replaces the previous selection.

Reference audit: `unit/weapons_catalogue.c` supplies the four 5/10/15 range
profiles with a four-hex minimum and two damage per missile. The mode flags are
used for selection, supply and display; the range and damage functions do not
read them. The rewrite retains these observed profiles for all three ammunition
labels. It does not add tabletop ER/HE range or damage rules absent from this
reference. ATMs use their own cluster rows from `unit/mech_build.c`, and each
missile produces a two-point packet through the common salvo resolver.

This audit also corrected Streak LRM packet sizing. The reference's
`weapon_catalogue_cluster_size` grants five-missile groups only to IDF/MRM/rocket
weapons with one-point damage. Streak LRMs lack those flags, so each missile
gets a separate hit location. The shared packet resolver and tests now preserve
that rule even under Angel confusion, while reusing Clan LRM tables to count
hits. Existing Streak firing and 49-pair AMS acceptance were rerun.

ATM tests exercise all four sizes and both selected ammunition types on all
seven chassis, native/Lua state equality, supplies, heat, rollback and restart.
AMS checks cover all 49 attacker/defender pairs. Additional tests compare range
and damage across the mode labels, verify catalogue and stock facts, reject
missing ammunition, test eligible non-ATM launchers, authority and switch guards,
and verify mode exclusivity. Integration acceptance remains open for MML and
the wider remaining command/configuration/scenario audit.

Validation passes 222 distinct affected tests: 37 in `target/atm-acceptance.log`
and 188 in `target/atm-final.log`, with three ATM tests repeated between those
runs. The latter includes all 177 library tests, four ATM integration tests and
seven help tests. `target/atm-streak-clusters.log` separately confirms the
corrected shared Streak packet test. All-target Clippy with warnings denied,
formatting, diff checks, Lua declaration mirroring and the unchanged reference
check pass. The last full-suite baseline remains the 2,062-test Gauss run.

## Functioning MML combat

The user explicitly selected functioning MML rules instead of preserving the
reference's missing missile-hit rows. IS MML-3/5/7/9 are now simulated Rust
weapons, retaining stock IDs 126–129 and dedicated ammunition IDs 318–321.
The simulated catalogue contains 153 identities; the stock catalogue still
contains 575 parts.

Normal MML ammunition uses the SRM profile: no minimum range, 3/6/9 bands,
two damage per missile, and individual missile hit locations. `MML_LRM`
ammunition uses minimum six, 7/14/21 bands, one damage per missile, and groups
of at most five damage. The published MML ammunition capacities and profiles
are recorded in the [BattleTech miniatures rules equipment table](https://battletech.com/downloads/CBTMiniRules_Final.pdf).
The 3/5/7/9 cluster columns use published BattleTech cluster-hit data
([Core Rulebook, cluster table, page 74](https://www.scribd.com/document/1069122135/BattleTech-Core-Rulebook));
existing launcher cluster columns retain their separate game behavior.
Full-ton SRM capacities are 33/20/14/11 salvos; LRM capacities are 40/24/17/13.
Half-ton sizing applies after choosing the family.

Native `mml <selection>`, Lua `btech.unit.mml(unit, pilot, index)`, and the
public Rust control toggle the same saved ammunition selection. Ordered weapon
selection, readiness, authority, publication rollback and exact matching-bin
selection reuse the common controls. Missing supplies reject firing without
switching families. `weaponspecs` and its Lua report show labelled SRM and LRM
profiles for each MML identity.

Aim, unit and terrain damage, AMS interception, bin normalization, ammunition
mass, dumping and internal ammunition hazards consume one shared ammunition
profile. No separate Mech/vehicle MML firing implementation was added. Observer
checks now take installation indices so two identical launchers can have
different ammunition families; only MML LRMs admit indirect fire. Hotloaded MML
critical explosions use the selected supplied family. Other launchers retain
their existing supply rules.

Acceptance covers all four sizes and both families across all seven supported
chassis, native/Lua equality, late callback rollback, real damage packets,
heat/supply expenditure, restart, command guards, missing selected supplies,
SRM/LRM specification rows, and ammunition criticals. AMS coverage uses both
families across all 49 supported shooter/defender chassis pairs. Shared cluster
tests exercise hotload and glancing rolls plus interception over 256 seeds.
Mixed Mech/vehicle observer tests verify SRM rejection and LRM admission.

Long-range special rounds are described in the next section. This delivery does not close the
broader command/configuration/scenario acceptance audit.

Full-suite verification: 2,079 distinct passing tests across
`target/mml-full.log` (325 passing tests before the cargo target),
`target/mml-remaining.log` (84 passing tests from cargo through digging),
`target/mml-duel.log` (one passing duel test on retry), and
`target/mml-final-suite.log` (1,669 remaining tests plus doc tests).
Every integration-test target in Cargo metadata has a passing result.
The GOD stock-limit fixture now selects a bounded laser family exceeding 20
part types instead of requiring the entire growing catalogue to fit the
communication output budget; production limits are unchanged. The live-duel
test initially exhausted its pilot-recovery wait and passed unchanged on an
isolated retry. That intermittent lifecycle-test failure remains an audit item.
All-target Clippy with warnings denied passes (`target/mml-clippy.log`). The
source-order cleanup it requested was followed by the focused observer library
test (`target/mml-spotter-final.log`). Formatting, diff checks and the Lua type
mirror comparison pass. The `btmux-khi` reference tree remains unchanged.

## MML LRM special rounds

An `MML_LRM` bin may also carry one LRM-compatible special round: Artemis IV
(`Artemis/Mine`), Narc (`Narc/Smoke`), `Swarm`, `Swarm1`, `Sguided` or `Stinger`.
Each pairing is its own ammunition mode (`mml_lrm_artemis`, `mml_lrm_narc`,
`mml_lrm_swarm`, `mml_lrm_swarm1`, `mml_lrm_semi_guided`, `mml_lrm_stinger`), so
bin matching, persistence, inspection bits and Lua projection work exactly as
they do for other modes. `BattleAmmunitionMode::munition` gives the round for
rules that key on it, and `is_mml_lrm` gives the family for range, damage,
five-point grouping, indirect fire and ammunition hazards.

SRM-family MMLs keep Artemis, Narc, Inferno, smoke and mine rounds. They still
refuse LRM-only Swarm, semi-guided and Stinger rounds, and the LRM family refuses
SRM-only rounds such as Inferno. The round controls (`artemis`, `narc`, `swarm`,
`swarm1`, `sguided`, `stinger`) choose within the currently selected family.
The `mml` control switches family and keeps a round both families carry; if the
new family cannot carry the round, it falls back to that family's normal round.

Guidance fallbacks keep the family. Blocked Artemis or Narc guidance fires plain
LRM rounds, a Narc beacon upgrades Narc rounds to the LRM Artemis cluster bonus,
and losing an Artemis controller drops only the round selection. Unit tests
cover flag parsing in either order, family combination rules and LRM grouping
under guidance. `tests/btech_mml.rs` covers selection, rejection, firing,
Stinger target restrictions and restart on every supported chassis.

## Loose-inventory consistency cleanup

`FIXSTUFF` and Lua `btech.inventory.fix(actor, object)` now share a Wizard-only
inventory cleanup action. This is the stock-consistency command described by
`economy/econ_cmds.c::economy_manifest_repair`, not the deferred unit-repair
system. Native requests use the operator's current location and ignore arguments.
The action removes unknown identifiers and the eight structural placeholders
(EndoSteel, FerroFibrous, TripleStrengthMyomer, StealthArmor, heavy/light
FerroFibrous and split-critical proxies). Manufacturer identities remain separate,
including branded non-weapon stock using the ordinary name fallback. Signed
matching records are consolidated, nonpositive totals dropped, and quantity
overflow rejected before mutation.

The report exposes original/new row counts and retained item totals. Cleanup,
immediate unit-load reconciliation and the private summary commit together;
notification failure or a later Lua callback failure restores the state and
staged output. Installed equipment and armor are unaffected. Empty inventories
are removed from storage. No startup, cockpit, cargo-command configuration or
unit-repair prerequisite is introduced.

The cargo fixture's substitutions previously looked for `Tracked` while the
actual template uses `{ Track }`. Its wheel, hover and stationary cases therefore
repeated tracked vehicles. The substitutions now produce the intended chassis,
including zero maximum speed for stationary units. All 20 cargo tests pass with
that corrected seven-chassis coverage. Cleanup tests verify native/Lua equality,
restart, unknown room stock, structural filtering, retained brands, idempotence,
authority, switch rejection, missing holders and failed-summary rollback. The
library test separately exercises duplicate/signed rows and overflow.

Acceptance: 229 distinct affected tests — 180 library and five access tests in
`target/inventory-cleanup-acceptance.log`, followed by 20 cargo, 17 command and
seven help tests in `target/inventory-cleanup-final.log`. Room-crew setup and transactional summary staging were corrected before the
final cargo run. This
is targeted evidence after the 2,079-test suite verification above; it does not
close the remaining operator/configuration/scenario audit.

All-target Clippy with warnings denied passes (`target/inventory-cleanup-clippy.log`),
as do formatting, diff checks and the Lua declaration mirror comparison. The
reference tree is unchanged.


## Map environment controls

Wizard `SETCOND gravity temperature [vacuum [underground]]` and Lua
`btech.map.environment(actor, map, conditions)` now share one transactional
map-environment action. Gravity accepts 0–255, temperature −128–127, and native
optional flags accept 0 or 1. Lua uses boolean flags and rejects unknown fields.
The command follows the reference handler's actual argument order. Underground
remains set when omitted or passed as false, matching that handler. Unrelated
map flags are preserved; special-condition flags are derived from gravity,
temperature and vacuum. Lighting and visibility retain their separate API.

Changes reach existing movement, heat, jump and underground-flight consumers
without copying environment state into individual units. An active jump retains
its route and reads the changed gravity on subsequent updates, including after
restart. State and private confirmation roll back together on output failure;
a later Lua callback failure also restores both.

Vacuum breach damage is still missing. The vacuum flag is persisted, but the
reference's armor/internal-damage breach checks and resulting equipment/crew
consequences require implementation. The audit also identified special-condition
damage-path dice consumption as part of that remaining work. These controls do
not establish complete environment-combat parity.

Acceptance: 215 distinct affected tests — 181 library, five access and 17 command
tests in `target/map-environment-acceptance.log`, plus five environment and seven
help tests in `target/map-environment-final.log`. The five environment tests were
rerun after adding rejection of unknown Lua fields
(`target/map-environment-controls.log`). Coverage includes all seven supported
chassis, native/Lua equivalence, authority and input rejection, preserved flags,
rollback, live heat/movement effects, underground takeoff restrictions and
in-flight gravity changes across restart. All-target Clippy with warnings denied
passes (`target/map-environment-clippy.log`). This is targeted evidence following
the complete-suite MML verification above; the broader integration audit remains
open.


## Vehicle vacuum breaches

Vacuum checks now run inside the existing vehicle armor and internal-damage
transactions for tracked, wheeled, hover and stationary vehicles and VTOLs.
Penetration that leaves a surviving section breaches directly. Armor-only hits
and internal-only explosions check 10+ on 2d6. The latter checks consume the
victim's random stream whenever special conditions are enabled, including
non-vacuum maps. Zero damage, combat-safe damage and destroyed-section paths
retain their existing early exits. Already breached sections still consume
eligible probabilistic checks, but do not repeat the breach notification.

A durable section set disables equipment without marking physical criticals
lost, removing structure, expending ammunition or killing crew. Operational
checks now distinguish unavailable equipment from physically destroyed slots;
weapon and critical diagnostics report disabled equipment. Section weapon
timers and unjam attempts are cancelled, and electronics reconcile through their
existing shared functions. Breached ammunition remains stored and contributes
mass, but cannot supply a weapon. Leaving vacuum does not restore equipment.
Snapshots reject breach locations absent from the vehicle design.

The earlier audit's proposed ground-vehicle destruction consequence was
incorrect: the reference equipment-loss routine returns early for vehicle
classes, before that later branch. The implementation follows the observable
vehicle behavior, including surviving hull and crew. Mech vacuum breach effects,
falls, cockpit casualties and Mech special-condition damage dice remain required
work; this increment does not establish complete environment-combat parity.

Focused acceptance includes nine vehicle armor-damage tests in
`target/vacuum-armor-final.log`. New scenarios exercise all five supported vehicle
chassis, penetration versus random checks, threshold boundaries, repeated
breaches, internal explosions, combat safety, blocked firing, disabled supply
and electronics, equipment diagnostics, invalid snapshots and restart after leaving
vacuum. The initial broad run caught an AMS capability regression: treating disabled
mounts as physical losses prevented a second intact mount from intercepting.
The whole-unit capability check again distinguishes physical loss from per-mount
availability; the latter excludes breached mounts during selection. The new
vehicle AMS regression case and the two existing Mech movement cases verify that
distinction. Recharge cancellation and active ECM shutdown are also covered.
Full-suite acceptance: 2,093 distinct passing tests. The first run
(`target/vacuum-suite.log`) passed 631 tests before the movement target reported
the two AMS regressions. After correction, all 154 remaining integration targets
passed 1,462 tests (`target/vacuum-remaining.log`), including all 354 movement
tests. A Cargo metadata comparison confirms passing results for every one of
236 integration targets. Partial passes from the failed movement run are not
counted twice.

Final library, booster and nine vehicle armor-damage tests pass in
`target/vacuum-final-checks.log`; these include the final timer/electronic-state
and AMS assertions. Documentation tests pass (`target/vacuum-doc-tests.log`), as
does all-target Clippy with warnings denied (`target/vacuum-clippy.log`).
Formatting, diff checks and the Lua declaration mirror comparison pass. The
`btmux-khi` reference tree remains unchanged. The integration goal stays open.


## Mech vacuum breaches and shared exposure effects

Biped and quad damage now use the same special-condition vacuum trigger as
vehicles. Surviving armor penetration breaches directly; armor-only hits and
surviving internal-only explosions check 10+ on 2d6, consuming the victim's stream
on all special-condition maps. Checks occur before transfer to the next section.
Previously disabled sections do not repeat their exposure effects.

Water and vacuum share `section_exposure::disable_section` and the public
`BattleSectionExposureReport`, with an explicit water/vacuum cause. This replaces
the water-specific report type without a compatibility alias. Flooding admission
and its existing report collections remain separate; vacuum consequences travel
with `BattleImpactReport.exposures` through direct and grouped damage. Their falls
and reactor effects use the same recursive casualty-publication functions.

Mech vacuum state is distinct from flooding but shares equipment availability,
engine/heat-sink/jump-jet loss and support rules. Exposed bins are emptied without
an ammunition explosion, and slots remain disabled rather than physically lost.
Breached legs force ground falls. Losing the last working jets during flight
uses the existing airborne balance and fall path. Core exposure shares reactor
instability policy. Head exposure destroys the unit and uses the established IC
casualty evacuation transaction; a failed evacuation restores damage, dice,
occupants and staged output. Leaving the environment does not repair equipment.
Snapshots reject lost/contradictory breached sections or retained ammunition in
new Mech breaches.

Eight focused tests pass in `target/mech-vacuum-final.log`: biped/quad equipment
and restart, random thresholds and special-map dice, immediate support falls,
IC cockpit evacuation and rollback, last-jet flight interruption, surviving
internal explosions, core engine loss with reactor explosions enabled/disabled,
and cancellation of a prone Mech's stand-recovery timer. The last-jet test names
the actual surviving jet when another slot in that section was already lost.
Negative snapshot tests use world validation, matching the persistence boundary;
Mech JSON decoding alone does not validate state. Low-level damage rejects an IC
breach that lacks casualty publication.

The full run passes 2,100 tests (`target/mech-vacuum-suite.log`), including every
one of 237 integration targets and documentation tests. Final changes were
followed by 321 passing tests in `target/mech-vacuum-final.log`: 181 library,
five crew, six impact, eight Mech vacuum, seven reactor-instability, 105 surface
and nine vehicle armor-damage tests. The additional stand-recovery case brings
the distinct verified total to 2,101. All-target Clippy with warnings denied
passes (`target/mech-vacuum-clippy.log`). Formatting, diff checks and the Lua
declaration mirror comparison pass. The reference tree is unchanged.

This does not close the remaining command/configuration/scenario integration
audit. The shared exposure report and availability methods live together in
`section_exposure.rs`; water and vacuum admission remain in their respective
modules.


## Cloud boundary state and unit contacts

Maps now own the existing signed-16-bit `cloudbase` database column through
`StoredBattleMap.cloud_base`. New maps default to 200, zero disables obstruction,
and updates preserve the value across restart and terrain reload. Database
loading uses checked integer conversion instead of silently narrowing corrupt
values. No schema extension or migration shim was introduced.

Wizard `@btech map-cloud <map>=<altitude>` and Lua
`btech.map.cloud_base(actor, map, altitude)` share the setter. Callback rollback
restores map state, invalid/overflow input is rejected, and the Lua declaration
mirrors describe the control. This setting remains separate from SETCOND's real
gravity/temperature/vacuum/underground grammar.

Both world-backed optical query APIs apply one cloud policy across all supported
chassis. Visual, light-amplification and infrared contacts cannot cross the cloud
boundary; equality belongs to its upper side. Radar and other non-optical modes
retain their own policies. Contact acquisition and targeting consume these
shared optical queries. Tests cover both directions across all 49 supported
chassis pairs, zero/negative/upper boundaries, native/Lua agreement, authority,
signed limits, callback rollback and persistence. The test fixture gives landed
VTOLs the raised terrain's altitude as well as changing the underlying tile.

This increment covers unit-to-unit cloud obstruction. The separate terrain-only
query behavior is covered in the following terrain-cloud entry. Broader
command/configuration/scenario parity remains open.


Cloud acceptance: 269 tests pass in `target/cloud-acceptance.log`, covering the
library plus BattleTech persistence, cloud controls, terrain, scanner chassis,
VTOL flight, mixed and vehicle sensors, Mech/vehicle aim, vehicle scanners,
visibility, radar, infrared, commands and help. The fire-spread test's hand-built
map was updated to include the newly owned cloud field. All-target Clippy with
warnings denied passes (`target/cloud-clippy.log`), as do formatting, diff checks
and the Lua declaration mirror comparison. The reference tree remains unchanged.
This is targeted evidence after the previous 2,101-test suite verification,
not a new complete-suite run.


## Terrain cloud visibility

The shared terrain-visibility path now applies the reference's terrain-only
cloud rule: a nonzero cloud base obstructs an observer strictly above it when
primary and secondary sensor modes differ. The gate applies to every mode in
that mixed pair, including electromagnetic sensors. Matching sensor modes do
not use this gate. Equality remains visible. This is deliberately distinct from
the unit-contact rule, which filters three optical modes across the boundary.

The check lives with the other cloud policy in `clouds.rs`, and
`hex_visibility` applies it before evaluating either role. Terrain firing, aim,
map rendering, scanning, artillery observation, coordinate spotting, building
contacts and terrain observer messages already consume this shared path.
Existing power/range checks and the operator clairvoyance override retain their
ordering. Queries do not acquire occupants, draw dice or change unit state.

The four cloud tests pass in `target/terrain-cloud-tests.log`. New coverage uses
all seven supported chassis, positive and negative cloud bases, strict/equal
altitudes, matching and mixed pairs, electromagnetic selection, restart and
read-only state equivalence. Native and Lua coordinate fire reject cloud-blocked
shots without spending state, and both succeed identically after clouds are
cleared. Late Lua failure restores a permitted shot. Operator visibility bypass
and Lua terrain aim are covered too.

Broader acceptance passes all 667 tests in `target/terrain-cloud-acceptance.log`,
covering the library, terrain fire, artillery, spotting, scans, visibility,
buildings, contact display, terrain, motion and help. All-target Clippy with
warnings denied passes (`target/terrain-cloud-clippy.log`), as do formatting,
diff checks and the Lua declaration mirror comparison. The reference tree
remains unchanged. This is targeted validation, not a new full-suite run.

## Seasonal map ice controls

The operator catalogue audit found no native `ADDICE` or `DELICE` handlers.
Both now use `map_ice.rs` with Lua `btech.map.add_ice(actor, map, percentage)` and
`btech.map.remove_ice(actor, map, percentage)`. Native commands operate on the
wizard's current map; Lua accepts an explicit map. Both require wizard authority
and one signed integer percentage. Out-of-range percentages retain the reference
threshold semantics rather than being clamped: eligible terrain still draws a
percentage die, even when the threshold guarantees rejection or acceptance.

The read-only contract is `movement/mech_ice.c` and its map command registrations.
Only ordinary water freezes. Growth counts adjacent water and bridges, excludes
ice and high water, and uses the original shoreline throughout the pass. Zero or
one qualifying neighbor needs no extra roll; two through four require a d6 above
the count; five or six prevent growth. Melting counts current ice neighbors:
up to four need no extra roll, while five or six require one on a d3. Earlier
melts expose edges to later cells. Both passes traverse columns before rows.
The Rust implementation uses a retained terrain snapshot for growth instead of
an intermediate terrain symbol. Freezing preserves submerged unit altitude.

Each decision commits the existing map-owned random stream before occupant
consequences can consume additional map dice. Melting invokes the ordinary
surface-break action, sharing Mech/vehicle falls, flooding, character handling
and notifications with combat. There is no second chassis-specific ice resolver.
An outer checkpoint rolls back the entire pass and staged output on any failure,
including after earlier hexes have changed. Reports return changed coordinates
and ordered fracture details. Help and mirrored Lua declarations are updated.

Six focused tests pass in `target/map-ice-tests.log`. Coverage includes all seven
supported chassis, exact agreement with combat fracture results, native/Lua
agreement, late Lua rollback, persistence, submerged Mechs, growth versus melt
ordering, signed percentage thresholds and dice consumption, invalid arguments,
authority, failed summary publication and failed occupied-hex publication after
an earlier unoccupied melt. The IC tracked-vehicle scenario preserves the normal
surviving crew in the flooded vehicle; vehicle disablement alone is not a crew
death or immediate evacuation. Broader validation is recorded below.

Continue the map/operator catalogue audit, including the `ADDHEX`, `@MAPEMIT`,
`CLEARMECHS`, map resizing and map asset saving contracts and their existing
adapters. Delivery of these two ice commands does not establish completion of
the catalogue, configuration or complete-scenario acceptance work.

Seasonal ice acceptance: 706 distinct tests pass. The first ten targets pass
682 tests in `target/map-ice-acceptance.log`; the command catalogue's expected
list then needed its two new names. After that correction, all 17 command tests
and seven help tests pass in `target/map-ice-final.log`. The combined coverage
includes the library, all new ice tests, clouds, environment controls, terrain
firing, 354 movement tests, 105 Mech/mixed surface tests and vehicle surfaces.
All-target Clippy with warnings denied passes (`target/map-ice-clippy.log`),
as do formatting, diff checks and the Lua declaration mirror comparison.
The reference tree remains unchanged. This is targeted validation, not a new
full-suite run or a claim of completed integration.

## Live terrain editing and occupied-map commits

`ADDHEX x y terrain elevation` and Lua
`btech.map.set_hex(actor, map, x, y, terrain, elevation)` now share a wizard-only
live edit action. The native command uses the operator's current map. The
reference contract is `map/map.c::map_addhex`: use the first terrain symbol,
accept `.` as grassland, take the absolute elevation magnitude and cap at nine.
The Rust action safely handles the minimum signed integer and rejects unknown
terrain symbols rather than introducing undecodable map cells.

The edit returns previous and resulting base tiles. It preserves unit altitude,
movement/flight state, equipment, dice, overlays and independently stored map
objects. Removing or lowering a bridge clears an obsolete hovercraft routing
hint while preserving its physical height. This action does not execute a
combat fracture or initiate falls. `DELICE` remains the seasonal operation that
melts ice with occupant consequences. Map facts are read live by subsequent
movement and visibility queries.

`terrain_edit::replace_hex` now owns the shared position-preserving mutation for
both live edits and ice growth. Anatomy-specific access is limited to retaining
implicit altitude and the hovercraft bridge hint; there is no second terrain
algorithm per chassis. Existing explicit flight/fall/ground altitude stays intact.
State, private confirmation and callback effects share an action checkpoint.

The audit found that the persistence validator still rejected occupied-map
terrain changes except for a small list of combat reductions. In particular,
newly added ice growth would fail to save once the occupied map already existed
in the database. The earlier ice restart tests saved newly created maps and
therefore did not exercise that comparison. The serializer now accepts validated
same-sized live terrain changes. The asset-reload operation continues to enforce
its own empty-map requirement. A final relational diff cannot distinguish an
asset reload from a live edit, so reload admission is kept at the domain boundary
instead of encoding terrain-operation guesses in persistence. Map resizing
remains separately guarded.

Five new integration tests pass alongside all six ice tests in
`target/terrain-edit-tests.log`. The new tests edit all fifteen supported terrain
kinds on all seven supported chassis, comparing native and Lua state, retaining
physical altitude and all other unit state, and saving/reloading each change
against an already-persisted occupied map. The ice regression separately saves
water before freezing it, then commits and reloads both growth and melting on
all seven chassis. Further coverage includes occupied asset-reload rejection,
authority, bad coordinates/symbols/arguments, signed elevation extremes, overlays,
late Lua rollback, failed confirmation, hovercraft bridge edits and active jumps.
The parser has a library test for every terrain symbol and numeric argument
bounds. Help and mirrored Lua declarations describe the new operation.

Full-suite validation follows below. The remaining map/operator catalogue and
integration acceptance review are still open.

Next confirmed map-command gap: `@MAPEMIT` has no native registration. The
reference `map/map.c` handler trims leading spaces, rejects empty text, calls
`ui/mech_notify.c::map_broadcast`, and confirms `Message sent!`. The audience is
map-slot occupants whose units are started, with no LOS/contact requirement.
This is distinct from the existing subject-relative observer broadcast and
needs a shared native/Lua operator action with transactional notification tests.

Live-terrain acceptance: all 2,117 tests pass in `target/terrain-edit-full.log`,
covering 182 library tests, all 240 integration targets and doc tests (zero).
The initial full run stopped at the access catalogue's missing wizard entries
for ADDHEX/ADDICE/DELICE; those independent permission expectations were added,
and the subsequent entire suite passed. All-target Clippy with warnings denied
passes (`target/terrain-edit-clippy.log`). Formatting, diff checks and the Lua
declaration mirror comparison pass. The reference tree remains unchanged.
This refreshes full-suite evidence while leaving the remaining integration
requirements open.

## Operator map broadcasts

`@MAPEMIT` and Lua `btech.map.emit(actor, map, text)` now share one wizard-only
map broadcast action. Native requests use the operator's current map. The action
removes leading spaces, rejects empty/NUL messages, captures eligible units in
persisted map-slot order, sends to cockpit occupants and privately confirms
`Message sent!`. The returned Lua array identifies eligible units rather than
counting players; a unit with no occupants can still belong to the audience.
The action does not mutate battlefield state or consume random draws.

The read-only `map/map.c` and `ui/mech_notify.c` contract requires running units,
conscious crews and no temporary sensor blindness. No LOS, contact, radio or team
check applies. Mechanical cockpit stun is distinct from unconsciousness and does
not suppress this message. The existing shared consciousness predicate was moved
from `sensor_flash.rs` to `crew.rs`, and both features reuse it. Unit selection
uses the existing mixed-chassis map-slot adapter. Notification traversal excludes
the unit container itself while including every direct cockpit occupant; a player
standing directly in the map room is outside this audience. The reference's
optional dropship turret-room forwarding occurs under a separate temporary combat
context; dropship units remain outside this integration scope.

The action checkpoints output so a later recipient/confirmation failure or an
outer Lua callback failure cannot leave an earlier cockpit message queued.
Native parsing performs its ordinary trailing-space normalization; the shared
API itself preserves trailing text. Native/Lua tests use equivalent parsed input,
with direct API coverage for leading-space removal and trailing-space retention.

Three focused tests pass in `target/map-emit-tests.log`, with the strengthened
slot-order and empty-cockpit coverage included in broader acceptance below. The
matrix covers all seven supported chassis in running, stunned, shutdown, starting,
blinded, assigned-pilot recovery and empty-crew recovery states. It compares native
and Lua recipients/messages after persistence restart, checks exact unchanged
battlefield state, clears acquired contacts, and verifies late callback rollback.
Further cases cover passengers, map-room bystanders, empty maps, authority,
invalid switches, empty/NUL messages and failed confirmation after two cockpit
messages have been staged. Help, Lua declarations, native catalogue and independent
wizard permission expectations are updated.

The remaining map/operator catalogue and complete integration review remain open.

Next map-operator audit item: `CLEARMECHS` delegates to
`integration/debug.c::map_shutdown_units`, ignoring its catalogue's optional
argument. It visits map slots, reports each unit, invokes privileged shutdown,
resets its position, removes membership, clears the dynamic membership list and
confirms `Map Cleared`. Compare this with the existing Rust unit-removal action,
including airborne consequences, towing, pilots, cleanup and persistence, before
adding a bulk wrapper.

Map-broadcast acceptance: all 300 targeted tests pass in
`target/map-emit-acceptance.log`, covering the library, new broadcasts, reactor
flashes, crew and vehicle character state, radio/C3i, visibility, both placement
adapters, commands, access and help. The broadcast matrix also reverses persisted
slot order relative to object IDs and verifies that an empty cockpit remains an
eligible unit without delivering to its former occupant. All-target Clippy with
warnings denied passes (`target/map-emit-clippy.log`). Formatting, diff checks,
Lua declaration mirror comparison and the unchanged reference-tree check pass.
This is targeted evidence after the 2,117-test full-suite run recorded above;
it does not close the remaining integration acceptance review.

### Shutdown consequences prerequisite for bulk map removal

The `CLEARMECHS` audit exposed two gaps in ordinary shutdown: ground vehicles
halted without a high-speed tumble, and the Mech host adapter did not retain
fall/stacking reports for character publication. Shutdown now retains those
reports and publishes injury, secondary impacts and crew evacuation under one
world/output checkpoint shared by native commands and Lua. Mechanical callers
continue to use the same power transition without owning host publication.

All supported chassis use the existing fall rules above positive actual speed
10.75; equality and reverse speed do not trigger the reference's moving-shutdown
branch. Jumping Mechs and elevated mobile vehicles enter their existing descent
state instead. Startup abort skips both branches. Vehicles reuse vehicle fall
packets, while Mechs retain the existing stacking resolver. Character-enabled
Mech impacts are selected only for in-character units. The descent transition
accepts an airborne carrier with an external load; administrative descent still
requires detached tow cables, and a carried target follows its carrier.

New shutdown tests cover native/Lua equivalence across seven chassis and speed
boundaries, callback rollback, saved airborne descent, startup abort, lethal
character injury and rollback when evacuation cannot complete. A towing test
covers all three representative carried chassis, fractional carrier altitude,
restart and deterministic subsequent movement. The existing VTOL controls test
now gives its constructed aircraft a valid map container before shutdown.

`CLEARMECHS` itself and the remaining integration acceptance audit remain open.

Final shutdown-focused acceptance passes all 71 tests in
`target/shutdown-final-tests.log`: shutdown, towing, VTOL crash and VTOL flight.
All-target Clippy with warnings denied passes in `target/shutdown-clippy.log`.
Formatting, diff checks and the Lua declaration mirror comparison pass. The
reference tree remains unchanged.

The broader `cargo test --no-fail-fast` run passes all 2,124 tests across the
library and 242 integration targets (`target/shutdown-full.log`), with doc tests
also passing. The final descent/towing implementation is additionally covered by
the 71-test focused run and all-target Clippy recorded above.

### Bulk map membership clearing

`CLEARMECHS` and wizard-only `btech.map.clear_units(actor, map)` now visit the
shared mixed-chassis map slots, report each member, apply the shared admitted
shutdown and publish fall/crew consequences, release either end of a tow, then
clear tactical membership. The final confirmation is `Map Cleared`; the native
argument is ignored, matching the reference's actual handler. Units and cockpit
occupants keep their game containment, while terrain, overlays, map settings and
map objects remain intact. Unpiloted running units are powered down too: removing
a unit cannot leave an engine running without a battlefield position.

Ordinary removal and bulk clearing reuse `placement::detach_membership`, including
contacts, target locks, C3/C3i membership, pilot assignments, building entry,
motion, flight/descent and chassis placement state. Ordinary removal retains its
container-move admission. Bulk clearing owns one world/output checkpoint covering
the entire pass, including secondary combat effects and evacuation.

Three new tests cover every supported chassis across stopped, starting, running,
moving and applicable airborne states; native/Lua equivalence; late callback
rollback; persisted removal and first-slot reuse; unchanged map data; authority;
empty maps; output-budget failure after an earlier removal; and both tow-slot
orders for Mech and airborne VTOL carriers. Lua declarations, help, command
catalogue and independent permission expectations are updated.

Read-only behavior references: `map/map.c::map_clearmechs`,
`integration/debug.c::map_shutdown_units`, and
`map/map_dynamic.c::battle_map_dynamic_destroy`. No reference-tree files changed.
The remaining map-operator catalogue, including resize/save behavior, and the
broader integration acceptance audit remain open.

Bulk-clear acceptance passes all 353 targeted tests in
`target/map-clear-acceptance.log` (library, map clearing, both placement adapters,
shutdown, towing, C3i, jumping, vehicle characters, commands, access and help).
The strengthened queued-output rollback test also passes in the final three-test
run (`target/map-clear-final.log`). All-target Clippy with warnings denied passes
(`target/map-clear-clippy.log`); formatting, diff checks, declaration mirror and
reference-tree checks pass. This extends the previous 2,124-test full-suite run
with targeted evidence; it does not complete the broader integration review.

### Terrain persistence with landing restrictions

The resize/save audit found that the terrain writer still classified type-9
landing restrictions as unowned map objects, although the dedicated landing
restriction adapter already loads and selectively saves them. As a result,
`ADDHEX` could succeed in memory and subsequently fail persistence on a map with
a previously saved landing restriction. The terrain guard now includes type 9
alongside the already owned mines, building routes and linked-map records. Other
unowned object records and opaque mine/hangar bits retain their existing guards.

A new regression covers all seven chassis on an occupied, already saved map,
including a type-9 row with an independent `data_short=77` field. It performs a
live terrain edit, saves and reloads, then clears units and reloads replacement
terrain. Both operations preserve the complete restriction and independent field.

The next resize implementation must account for the actual reference behavior:
`map_setmapsize` copies the overlapping effective tiles into a grassland-zero
rectangle, then `del_mapobjs` removes all map objects and linked-map markers.
Removing a building entrance clears its interior's return links. TYPE_BITS blocks
the operation. Rust must coordinate these owned records, unit coordinates and
in-flight events with the dimension/terrain writer rather than changing width and
height alone. Native resize/save commands remain unimplemented.

The save audit also confirms that `map_savemap` distinguishes temporary fire from
permanent fire, reveals terrain under smoke, removes stale fire/smoke, omits the
map-object flag, and writes optional environmental metadata. A plain serializer
of the current visible grid would not implement that contract. Future file writes
must also participate in the enclosing action's publication/rollback policy.

All 93 targeted terrain, seasonal ice, scanner/landing and persistence tests pass
(`target/map-object-terrain-acceptance.log`). All-target Clippy with warnings denied
passes (`target/map-object-terrain-clippy.log`); formatting, diff, Lua declaration
mirror and unchanged-reference checks pass. The integration goal remains open.

### Transactional map resize

`SETMAPSIZE width height` and wizard-only `btech.map.resize(actor, map, width,
height)` now copy overlapping visible tiles and fill the remainder with level
grassland. As in `map_setmapsize`, resizing clears map objects even when the
requested dimensions are unchanged: decoration timers, mines, landing restrictions,
entrances, entry points, exits and wrapping markers. Interiors referenced by removed
entrances lose their return links. Visible fire/smoke tiles survive as underlying
tiles when their timers are removed, matching the reference's copy-before-delete
ordering. Other map settings and valid unit positions remain unchanged.

Dimensions use the established Rust map range of 1 through 1000. Cropping a unit,
cargo point or active event outside the new rectangle fails world validation and
restores the entire action and output. Rust does not retain the reference's ability
to create a zero-sized grid or out-of-bounds runtime state. Opaque map-object/bit
records retain persistence guards; type-8 records are not interpreted or deleted.

Persistence now accepts validated dimension changes, crops dictionary-backed grid
rows beyond the new bounds and selectively updates retained coordinates. Terrain
reads continue rejecting unknown codes, missing cells and invalid coordinates.
Equal-area reshaping must bypass the conditions-only save path even when its
row-major tile vector is unchanged; this case is covered explicitly.

Tests cover all supported chassis across expansion and shrinking, native/Lua
identity, callback rollback, unchanged units, invalid bounds and clipped-unit
rejection. Additional tests cover equal-area reshaping, landing restriction removal,
preserved unrelated columns on retained cells, defaults on new cells, same-size
fire/smoke timer cleanup and reciprocal building exit removal. Each successful case
saves an existing durable map and verifies restart. Help, Lua declarations, command
catalogue and independent wizard permissions are updated. Map saving and the
remaining catalogue/integration audit remain open.

Resize acceptance passes all 122 targeted tests in `target/map-resize-acceptance.log`
(resize, terrain codec/persistence, live terrain edits, clearing, scanner/landing,
persistence, commands, access and help). The final three-test resize run also
passes with independent-cell-field checks (`target/map-resize-final.log`).
All-target Clippy with warnings denied passes (`target/map-resize-clippy.log`),
as do formatting, diff, declaration mirror and unchanged-reference checks.

### Map-save encoding

`StoredBattleMap::export_asset` now prepares the reference map-file text together
with ordered stale-effect coordinates for the enclosing save action. It does not
write files or mutate world state. Temporary fire exports as `>` (which reloads
as grass), smoke reveals the underlying tile, unflagged fire without a timer and
bare smoke export as grass and request cleanup, and flagged permanent fire stays
`&`. Smoke over underlying fire does not re-enter the fire branch. Grass exports
as `.`, elevation is retained, and the map-object flag is excluded from metadata.
Environmental metadata is present only when another flag remains, matching the
reference even when unflagged in-memory conditions differ from parser defaults.

Two tests cover all terrain symbols, permanent/temporary/stale effects, smoke over
fire both with and without the permanent-fire flag, exact rows and metadata,
parser round trips, unchanged world state and stable export after persistence.

The export is preparation for `SAVEMAP`, not a completed file-save command. The
next step is bounded asset-write staging in the existing runtime effect batch,
including nested savepoints, rollback, runtime inheritance and aggregate limits,
followed by atomic file replacement and completion/error publication after world
commit. The target must remain confined to the configured map directory. A late
Lua failure or failed world save must discard staged writes; final file failure
must preserve the previous destination and must not emit a success message.

All 20 export, terrain, live-edit and resize tests pass in
`target/map-export-acceptance.log`; all-target Clippy with warnings denied passes
in `target/map-export-clippy.log`. Formatting, diff and unchanged-reference checks
pass. File publication and the broader integration goal remain open.

### SAVEMAP publication

`SAVEMAP name` and wizard-only `btech.map.save(actor, map, name)` now use the shared
export to stage a complete file replacement and apply stale terrain cleanup through
the existing live-hex resolver. The Lua result means queued, not written. A new
`MapAssetWrite` effect participates in checkpoints, callback rollback, host rollback,
runtime inheritance and aggregate effect byte/entry budgets. Payload strings share
immutable storage across checkpoints. The existing `tempfile` dependency is now a
runtime dependency for atomic replacement.

The server publishes these effects only in its post-commit flush. Files are written
to same-directory temporary files, synchronized and renamed atomically. Existing
permissions are retained. Names resolve under the configured map directory; parent
traversal, absolute paths, outside-directory symlinks and destination symlinks are
rejected. Parent directories must already exist. Destinations are rechecked before
replacement. Failed pre-replacement IO leaves an existing file untouched, and a
success notice is emitted only after replacement. Existing world cleanup is already
committed if later file publication fails; that failure reports an error rather than
pretending to roll back the durable world. Prepared writes are not a durable job
queue and are not retried across process termination before publication.

Tests cover native/Lua preparation, late callback failure, unchanged destination
before publication, final bytes, invalid paths, symlink escape and destination
replacement between preparation/publication, aggregate budget rollback and runtime
inheritance. A live-server test injects a database failure into stale-fire cleanup:
neither the target file nor durable terrain changes and no success is announced.
Removing the failure and retrying saves the expected file and announces completion.

All 2,136 tests across the library and 246 integration targets pass in the full
`cargo test --no-fail-fast` run (`target/map-save-full.log`); doc tests also pass.
All-target Clippy with warnings denied passes (`target/map-save-clippy.log`).
Formatting, diff, Lua declaration mirror and unchanged-reference checks pass.
The remaining map/operator catalogue and full integration review remain open.

### Bridge generation during asset activation

The LOADMAP audit found that asset activation still omitted the reference's road
to bridge generation. `BattleMapAsset::generate_bridges` now supplies that shared
step to `map_from_asset`, so creation and terrain reload agree while raw parsing
continues to expose authored tiles. Flag 128 disables generation. Roads qualify
when opposite search directions reach water or ice within three steps on each
side; intermediate terrain must be road or bridge. High water is not an endpoint.
The reference's distance-difference check adds no restriction once both distances
are in 1..3, so six-step water-to-water spans can qualify. Elevation is unchanged.
The search retains the reference's coordinate adjustment when stepping into column
zero; ordinary movement-neighbor geometry would produce a different result there.
Road and bridge are equally traversable by this search, allowing an immutable
classification pass followed by one copy-on-write update.

Tests cover all endpoint-distance pairs from one through four, water and ice,
obstructed corridors, disable flags, idempotence, raw-source preservation, diagonal
and western-edge searches, creation/reload and durable restart. The landing scanner
test now distinguishes generated bridges from explicitly disabled generation.
The same loading-path audit fixed `reload_map` dropping a configured cloud base;
a nondefault cloud boundary now survives terrain reload and restart.

Native LOADMAP remains open. Its reference handler loads terrain before clearing
units for ordinary wizards, while actor #1 retains membership. This distinction,
new dimensions, object cleanup and post-load shutdown consequences must be composed
with the existing host actions; the simpler fixed-size unoccupied reload API does
not itself implement that command.

All 637 targeted bridge, cloud, jump, export, resize, movement, scanner, terrain and
vehicle driving/surface tests pass (`target/bridge-generation-acceptance.log`).
The final seven-test bridge/cloud run also passes with all three search axes
covered (`target/bridge-generation-final.log`). All-target Clippy with warnings
denied passes (`target/bridge-generation-clippy.log`); formatting, diff and
unchanged-reference checks pass. This extends the previous full-suite evidence;
LOADMAP composition and the broader integration review remain open.

### Native LOADMAP composition

`LOADMAP name` and wizard-only `btech.map.load(actor, map, name)` now parse and
activate an asset, clear map objects, replace dimensions/terrain/asset conditions,
and then apply the reference's operator policy. GOD (#1) retains map membership;
other wizards announce clearing and use the existing bulk shutdown/removal action.
The replacement terrain and gravity are therefore active before shutdown falls.
Physical altitude is retained through the shared live-hex resolver, including
under-bridge cleanup. Other map settings, including cloud base, remain unchanged.

Resize and loading now share `map_objects::clear` for decoration timers, minefields,
landing restrictions, building links and reciprocal interior exits. The existing
fixed-size unoccupied terrain reload delegates its replacement work to the same
asset replacement helper while retaining its own admission policy. No second
shutdown, movement or chassis-specific loader has been introduced.

Files are decoded before any world changes. Invalid files, out-of-range crops,
failed nested consequences and callback errors restore the entire world/output
checkpoint. A crop excluding a placed unit requires clearing/moving units first;
the reference's grid access aborts on such coordinates, which Rust does not emulate.
Opaque, unowned map-object and bit records retain their persistence guards. These
commands do not imply that those remaining imported representations are owned.

Tests cover all supported chassis through native and Lua entry points for GOD and
ordinary wizards, moving shutdown, retained altitude, changed dimensions and
conditions, cloud retention, late callback rollback, malformed/missing assets,
unauthorized actors and restart. A focused tracked-vehicle test loads water beneath
a moving unit and requires free-fall rather than an old-ground tumble, directly
verifying load-before-shutdown ordering. Help, declarations, command catalogue and
independent permissions are updated. The remaining operator catalogue and overall
integration review remain open.

All 278 targeted tests pass (`target/map-load-acceptance.log`): library, loading,
resizing, clearing, terrain, live edits, shutdown, towing, clouds, bridge generation,
commands, access and help. All-target Clippy with warnings denied passes
(`target/map-load-clippy.log`), as do formatting, diff, declaration mirror and
unchanged-reference checks. The broader goal remains active.

### Native SETLINKED

`SETLINKED` now enables the current map's existing wrapping state through
`set_map_wrapping`, preserving terrain, unit state and owned marker persistence.
It ignores its argument and reports `Map set to linked.` Repeated calls are
idempotent. There is no second wrapping implementation or Lua alias: trusted
scripts continue using `btech.map.wrapping`, and the existing administrative toggle
can disable wrapping. The native handler checkpoints state and output so a failed
confirmation cannot silently change the map. Wizard access is registered in both
the command catalogue and independent permission fixture.

Tests cover every supported chassis, native/Lua state equality, repeated calls,
unchanged units, persistence restart and confirmation-failure rollback. Existing
jump tests exercise wrapping route admission. Read-only reference:
`map/map_obj_commands.c::map_setlinked`. The remaining map/operator catalogue and
full integration audit remain open.

All 88 targeted tests pass in `target/map-link-acceptance.log`; all-target Clippy
with warnings denied passes in `target/map-link-clippy.log`. Formatting, diff and
unchanged-reference checks pass. The broader integration goal remains active.

### Native ADDBLOCK and signed landing restrictions

`ADDBLOCK <x> <y> <distance> [team]` and `btech.map.add_block` now add restrictions
through the existing circular landing checks and selective map-object persistence.
Both entry points share wizard authorization, coordinate validation, stable slot
allocation, confirmation and world/output rollback. Team zero exempts nobody;
other team values retain their full integer width. Negative radii remain inactive
restrictions, and radius zero blocks the exact center. Stored radii use signed
64-bit values to preserve the reference map-object scalar without narrowing.

Tests cover native/Lua equality, callback rollback, optional and full-width teams,
signed command boundaries, multiple slots, invalid coordinates and arguments,
authorization and restart. Imported radii at both signed 64-bit limits and the
unsigned 32-bit maximum load without truncation. Read-only reference:
`map/map_obj_commands.c::map_add_block` and `map/map.h::MapObject`.

All 111 targeted tests pass in `target/map-block-acceptance.log`, covering landing
checks, map loading/resizing, terrain editing, commands, access and help alongside
the new command. All-target Clippy with warnings denied passes in
`target/map-block-clippy.log`. Fire/smoke operator commands remain open: their zero
duration means a permanent decoration and requires extending the shared timer
representation. The overall integration goal remains active.

### Permanent fire and smoke markers

The shared decoration model now accepts zero remaining seconds as a permanent,
unscheduled marker. Such markers remain visible to the existing terrain consumers
but neither expire nor spread. Fire/smoke pending checks exclude them, and active
clocks on the same map leave them intact. A permanent fire cannot carry a spread
timer. Replacement and removal still use `set_map_decoration`, preserving the
underlying terrain and the existing shared lifecycle for timed effects. The owned
table permits zero lifetimes; negative stored lifetimes remain invalid.

The new lifecycle test covers idle ticks, mixed clocks, unchanged random state,
restart, replacement, removal and rejection of a contradictory permanent/spreading
definition. The existing corruption test now injects a negative lifetime instead
of the newly valid zero. All 21 targeted tests pass in
`target/permanent-decorations-tests.log` (terrain, export, resize and vehicle hex
fire). All-target Clippy with warnings denied passes in
`target/permanent-decorations-clippy.log`.

ADDFIRE/ADDSMOKE wiring remains open. The read-only reference audit confirms that
smoke uses the original signed integer duration as its event delay (delays below
one become one tick), while fire stores a clamped signed short and performs its
first spread after the wind interval before scheduling burnout. The local
post-insertion duration scaling does not change the inserted object. These timed
command boundaries need explicit handling and tests; the new permanent state
must not be used to collapse negative durations into permanent effects. The
broader integration goal remains active.

### Native ADDFIRE and ADDSMOKE

Both commands and Lua `btech.map.add_fire`/`add_smoke` now share one wizard action
for marker replacement, duration admission, confirmation and transactional
rollback. Off-map coordinates confirm without changing the map, matching the
operator handler. Zero stays permanent. Negative smoke expires after one tick;
positive smoke preserves the full signed command range. Shared decoration seconds
are now unsigned 32-bit values, including in selective persistence. Existing
artillery, woodland and blast producers convert their bounded durations into that
same representation.

Timed fire uses the signed-short upper limit and lasts through its first wind
spread, followed by at least one burnout tick. The shared fire lifecycle handles
all subsequent spread, smoke and burnout consequences. There is no duplicate
operator or chassis-specific simulation. Help, native ordering, independent
permissions and mirrored Lua declarations are updated.

Tests cover signed extremes, zero, first-spread boundaries, long smoke, exact
expiry ticks, restart replay, native/Lua equality, callback rollback, admission,
off-map no-ops and replacement revealing the original terrain. All 244 tests pass:
46 in `target/map-decoration-acceptance.log`, 182 library tests in
`target/map-decoration-library.log`, and 16 artillery/ammunition/fire-target tests
in `target/map-decoration-combat.log`. All-target Clippy with warnings denied
passes in `target/map-decoration-clippy.log`; formatting, diff, declaration mirror
and unchanged-reference checks pass.

The operator catalogue and overall integration audit remain open. Fire timing
when wind changes between scheduled spread events still needs a dedicated
reference comparison: these tests establish duration boundaries at constant wind,
not that broader event-scheduling claim. The goal remains active.

### Fire budgets and changing wind

Fire now keeps its spread budget separate from the pending event countdown using
its existing two timer fields: `remaining` is the budget while `next_spread` is
present, and becomes a burnout countdown after the final spread. A scheduled
spread keeps its deadline when wind changes. At that deadline the current wind
interval spends the budget and selects the next spread or burnout. A scheduled
burnout also keeps its deadline. This removes the previous per-second budget
reduction without adding a parallel operator lifecycle or additional saved state.
Short positive fires reach their first spread before entering one-tick burnout.

Restart tests change wind in both directions midway through a pending event and
verify the next budget, deadline and eventual expiry. Operator boundary tests now
check visible expiry separately from the fire budget. The permanent-marker test
allows smoke generated by the newly reached first spread to expire before checking
that only permanent markers remain. All 20 focused tests pass in
`target/fire-wind-final.log`. A full suite is being checked separately.

An additional signed-short boundary audit found a remaining mismatch for extreme
negative ADDFIRE durations: the reference's minimum short wraps positive when an
interval is subtracted. The Rust command currently normalizes all negative fire
budgets to one. A local compiler probe (`target/fire-duration-boundary.c`) confirms
-32768 becomes 32748 after subtracting 20, or 32708 after subtracting 60. This needs
an explicit signed budget representation and regression coverage; ordinary
negative and zero-duration tests do not prove that boundary. Overall integration
remains active.

### Signed fire budget boundaries

Extreme negative ADDFIRE durations now retain their clamped signed-short budget
until the first spread. At that event negative budget subtraction uses explicit
16-bit wrapping, matching the verified reference behavior; the resulting positive
budget continues through the same shared lifecycle. Nonpositive results schedule
one-tick burnout. No alternate negative-duration event loop was added.

The existing timer scalar is signed 64-bit so it can preserve both signed-short
fire budgets and the full unsigned 32-bit smoke range. Validation allows negative
values only for fire with a pending spread and rejects values outside the owned
range. SQL storage, artillery/woodland/blast producers and documentation use the
same representation. Zero remains permanent and independent of this boundary.

The new test covers both sides of the wrap boundary at 20- and 60-second intervals,
changing wind after admission and replaying the first event after restart. Native
and Lua command boundary tests preserve the initial signed values and still check
ordinary expiry. All 29 targeted tests pass in `target/fire-signed-tests.log`;
all-target Clippy with warnings denied passes in `target/fire-signed-clippy.log`.
Formatting, diff, declaration mirrors and unchanged-reference checks pass.

The full suite in `target/fire-wind-full.log` remains in progress against the
preceding wind-timing snapshot, before this signed representation change. Do not
report that run as a full-suite result for the signed-budget state. Map-object
listing/deletion and the broader integration audit remain open; the goal is active.

All seven help tests also pass in `target/fire-signed-help.log`, bringing this
change's targeted acceptance total to 36.

### Shared entrance-removal consequences

Removing one exterior building entrance now clears all return links from its
interior, matching `del_mapobj(TYPE_BUILD)`. Interior arrival points and other
exterior entrances remain. A remaining entrance does not recreate return links.
The whole-map object clearer now calls that same remover instead of implementing
its own reciprocal cleanup. Removing a nonexistent slot is a no-op.

The route-selection test previously expected fallback through a second entrance
after removing the first; it now requires the reference's missing-return-link
result. A new test saves multiple return links, removes one exterior entrance,
checks preserved arrivals and remaining inbound access, then saves/reloads both
maps and repeats removal. All 24 building route/entry, map resize and map load
tests pass in `target/entrance-removal-acceptance.log`. All-target Clippy with
warnings denied passes in `target/entrance-removal-clippy.log`.

DELOBJ itself remains open. Its type and coordinate selectors require the actual
coordinates/counts of LEAVE and LINKED records; current adapters intentionally
preserve some of those fields without loading them, so assuming origin coordinates
or one wrapping marker would be incorrect for imported maps. DECO/TBITS and listing
also need an ownership audit. The ongoing full suite remains the earlier
wind-timing snapshot. Overall integration remains active.

The broader surface run found two assertions still using the superseded fire
countdown model. They now require the unchanged budget before a spread, the new
budget after a changed-wind spread, and the separate one-tick burnout after the
calm first spread. Smoke ages during that burnout tick. All 105 surface tests pass
in `target/entrance-removal-surfaces-final.log`, for 129 targeted passes in this
turn. The older full-suite snapshot reports those two known assertion failures;
its remaining targets are still running. Do not describe that snapshot as green.

### Linked marker identity and coordinate ownership

Wrapping now derives from an ordered collection of linked-marker coordinates,
replacing the separate saved boolean. Imported duplicate markers and off-map
coordinates survive loading. A shared marker setter can update/remove one slot,
checks active-jump admission before changing boundary behavior, and preserves
other markers. The general wrapping toggle remains idempotent when enabling and
removes all markers when disabling. SETLINKED now adds a marker on every call,
matching the reference's insertion behavior; the earlier idempotency claim for
that native command is superseded.

Selective persistence owns slot identity and coordinates while preserving unrelated
payload columns on existing records. New records carry the reference's zero
object/scalar fields and enabled data flag. Reload keeps marker state; resizing
and whole-map object clearing remove it. Movement, building exit policy, VTOL
contact resolution and Lua inspection derive wrapping from the same collection.
Lua map inspection also exposes `linked_markers` with their actual slots and
coordinates.

Tests cover repeated SETLINKED insertion across supported chassis, equality with
the first Lua toggle, Lua marker inspection, duplicate imported coordinates,
selective coordinate edits preserving auxiliary fields, removal of one marker
without disabling wrapping, idempotent enable, all-marker disable and restart.
The LEAVE record-coordinate ownership and DECO/TBITS audit remain open before
DELOBJ/listing can be claimed complete. Overall integration remains active.

The older full wind-timing snapshot finished with 2,149 passes and two surface
assertion failures in `target/fire-wind-full.log`. Those exact assertions were
corrected and all 105 surface tests subsequently passed, as recorded above. That
snapshot predates signed fire budgets, entrance cleanup and this marker change;
it is not a full-suite verification of the current tree.

All 91 targeted tests pass in `target/linked-markers-final.log`. All-target Clippy
with warnings denied passes in `target/linked-markers-clippy.log`; formatting,
diff, Lua declaration mirrors and unchanged-reference checks pass.

### Return-link coordinate ownership

Building exits now use `BattleBuildingExit`, containing both selection coordinates
and the destination map. Imported coordinates are loaded and persisted rather
than assumed to be the origin. The existing destination-only setter preserves
coordinates; the full-record setter validates destinations through the same
admission path and permits out-of-map metadata coordinates. Travel continues to
resolve the reciprocal exterior entrance, independent of the return record's
coordinates. Validation, object cleanup, reload and Lua inspection consume the
same record type.

Selective persistence owns the coordinates and destination while retaining
auxiliary payload columns. Tests import non-origin coordinates, update them,
confirm destination-only edits retain them, reject missing/self destinations
without mutation, inspect them through Lua, verify unchanged travel destinations,
and save/reload the result. Existing entrance cleanup and map resize/load tests
continue to pass. All 26 targeted tests pass in `target/return-record-tests.log`,
with the final three route tests (including Lua inspection) also passing in
`target/return-record-routes-final.log`.

Linked and return records now expose the coordinates required for DELOBJ selection.
DECO/TBITS ownership and the actual deletion/listing commands remain open; this
change does not claim those commands are delivered. Overall integration remains
active.

All-target Clippy with warnings denied passes in
`target/return-record-clippy-final.log`; formatting, diff, declaration mirrors and
unchanged-reference checks pass.

### Generic DECO record ownership

The reference audit found no active authoring path for TYPE_DEC; its saved records
carry terrain to restore during explicit deletion and no automatic timer. Rust now
owns those records as `BattleStaticDecoration` rather than silently leaving them
outside map state. The shared setter validates coordinates, metadata remains
separate from visible terrain and thermal overlays, and selective persistence
preserves unrelated payload columns. Explicit fixed-size asset reload retains the
records; native map load/resize clearing removes them without restoring stale
terrain. Generic records no longer trigger the unowned-object terrain-save guard.

Tests import restoration metadata, verify it does not paint terrain or start
clocks, reject invalid edits atomically, write new/changed records while retaining
auxiliary fields, reload terrain, save/restart and clear records through resizing.
Malformed coordinates and restoration symbols are also checked. The targeted
map/terrain run passes all 17 tests in `target/static-decoration-final.log`.

TBITS is a two-bit-per-hex mine/hangar lookup cache in the reference. Its deletion
can disable valid definitions because gameplay consults the cache. Rust currently
derives lookups from definitions. A user preference question is pending on keeping
derived lookups versus reproducing cache-deletion behavior; this generic-record
work is independent of that choice. DELOBJ/listing and overall integration remain
open. Read-only references: `map/map_bits.c`, `map/map_obj.c`, and
`persistence/map_restore.c`.

The final generic-record test with corruption cases passes in
`target/static-decoration-record-final.log`. All-target Clippy with warnings denied
passes in `target/static-decoration-clippy-final.log`; formatting, diff and
unchanged-reference checks pass.

### Native DELOBJ and shared map-object deletion

`DELOBJ TYPE`, `DELOBJ X Y`, and `DELOBJ TYPE X Y` now dispatch through one action
shared with `btech.map.delete_objects`. Native type prefixes follow catalogue
order. The action handles fire, smoke, generic decorations, mines, building
entrances, return links, interior arrival points, linked markers and landing
restrictions. It requires a type or coordinate selector. Off-map selectors can
match authored linked/return metadata; unmatched coordinates report zero.

Deletion delegates to the existing typed removers. Generic decoration restoration
uses the same physical-altitude-preserving terrain edit as ADDHEX. Building removal
clears interior return links without including that reciprocal cleanup in the
reported count. Linked marker removal preserves wrapping while another marker
remains and retains active-jump validation. Whole-action world/output checkpoints
restore earlier removals and terrain changes after any failure.

Pending the optional TBITS preference, implementation continues with Rust's
existing derived mine/building lookups. An explicit TBITS request reports that no
separate cache object exists and leaves definitions active. Coordinate selection
counts actual owned objects, not a virtual cache. This is a stated design
assumption, not a claim that the user selected an answer. Imported unowned C fire/
smoke map-object rows and raw bit-table persistence still require an integration
ownership audit; runtime fire/smoke overlays use the existing owned timer table.

Tests cover all nine owned kinds, all selector forms, prefix matching, exact
counts, native/Lua equality, restart, out-of-map no-ops, authority and argument
rejection, callback rollback and confirmation-failure rollback after multi-kind
cleanup. Help, Lua declarations, command ordering and independent access fixtures
are updated. All 41 targeted tests pass in `target/map-delete-acceptance.log`;
all-target Clippy with warnings denied passes in `target/map-delete-clippy-final.log`.
Formatting, diff, declaration mirrors and unchanged-reference checks pass.

Native `LIST [MECHS | OBJS]`, UPDATELINKS and remaining map/operator coverage still
need review. Overall integration remains active.

### Native LIST and shared map reports

`LIST MECHS` and `LIST OBJS`, with case-insensitive prefixes, now use the same
read-only publication action as `btech.map.list`. Unit rows use shared mixed-chassis
slot order and battlefield labels, include the unit count and reference's nominal
250-position availability report, and do not perform sensor acquisition. Rust has
no independent stored `first_free` high-water mark, so it does not invent the
reference's optional database high-water diagnostic or invalid C-data pointers.

Object rows show owned type, slot, coordinates and useful timer/restoration,
mine, destination, direction or landing-restriction details. This is a typed Rust
operator report rather than the reference's raw `obj/dc/ds/di` storage dump;
unowned auxiliary database fields are not assigned fabricated values. Listing
and deletion share `object_positions`, so they enumerate the same records in the
same order. Their multi-object test fixture is also shared. TBITS remains the
previously stated derived-lookup assumption and has no synthetic listing row.

World/output checkpoints discard partial publication after output-limit errors or
callback rollback. Tests cover all supported chassis with deliberately reordered
slots, unchanged simulation/contact/random state, native/Lua output equality,
all owned object kinds and relevant details, argument/authority rejection, late
callback rollback and failure after some report lines have queued. Help, mirrored
Lua declarations, native ordering and independent permissions are updated.

All 35 targeted tests pass in `target/map-list-tests.log`; all-target Clippy with
warnings denied passes in `target/map-list-clippy.log`. Formatting, diff, declaration
mirror and unchanged-reference checks pass. UPDATELINKS, imported unowned records
and the broader integration review remain open; the goal is active.

### Authored map links and native UPDATELINKS

Authored child-to-parent placements and four cardinal arrival modes now persist
in `btech_map_links` and `btech_map_entrances`. `btech.map.link` reads configuration;
`btech.map.set_link` validates edits without changing runtime routes. Entrance
modes are absent, exact coordinate, or an inward edge offset. Offsets clamp to
current map dimensions; resizing retains authored intent, and rebuilding skips
placements or exact arrivals that no longer fit. Saves preserve inactive payload
columns and accept missing default-mode entrance rows.

Native Wizard `UPDATELINKS` and `btech.map.update_links` share an explicit-stack
traversal and the existing building-route setters. Children are visited in
ascending database order, while resulting entrance selection follows the
reference's prepended order. Shared removal clears stale reciprocal return links.
Cycles and the 1,024-level depth limit terminate descent without consuming the
Rust call stack; incoming building records are still counted. Root return and
arrival records are retained except where reciprocal removal clears them. Whole
world/output checkpoints make failed publication and late Lua callback failures
atomic across the entire rebuild.

Tests cover ordering, cardinal resolution, stale routes, cycles, depth bounds,
resizing, native/Lua equality, authority, rollback, selective SQL writes and restart.
The 43 distinct targeted tests pass across `target/update-links-acceptance.log`
and `target/update-links-final.log`; all-target Clippy with warnings denied passes
in `target/update-links-clippy.log`. Help has one Wizard-only UPDATELINKS article;
command ordering, access fixtures and mirrored Lua declarations are updated.
Formatting, diff and unchanged-reference checks pass.

A new complete suite is running in `target/update-links-full.log`; its result is
not yet established. Imported fire/smoke map-object ownership, raw bit-table
handling and the broader integration audit remain open. The integration goal
remains active.

### Imported fire/smoke ownership audit

Read-only checks of `persistence/map_restore.c::btech_special_load_map_objects`
confirm that restored FIRE/SMOKE rows have no scheduled decoration event. Their
saved `data_short` is not a remaining wall-clock lifetime. Import must not turn
it into a fresh spread deadline or expiry countdown. The terrain dictionary
already contains the visible marker. `map_terrain.c::map_real_terrain_get` uses
restoration metadata only when visible terrain is FIRE or SMOKE;
`map_buildings.c::map_underlying_terrain` falls back to the visible terrain when
no restoration record exists. `map_obj.c::find_decorations` selects the first
matching record in FIRE, SMOKE, DECO order, preserving ordinal order within a
kind. Duplicate coordinates can therefore be significant.

This is more than loading permanent overlays. The existing runtime overlay map
is keyed by tile and cannot retain duplicate source records or source ordinals.
The next ownership change must share the generic restoration record model across
all three stored decoration kinds, retain distinct record identities, and expose
both visible and underlying terrain consistently. Operator listing/deletion
must enumerate these records as well as runtime overlays without conflating a
source ordinal with a runtime tile index. Explicit removal restores recorded
terrain; whole-map object clearing preserves visible terrain. The shared new-
decoration operation must replace prior decoration records at the coordinate,
matching `map_obj.c` without reviving imported timers. Persistence must preserve
unowned payload fields on surviving rows and remove owned rows when cleared.

Acceptance needs imported duplicate records, visible/underlying terrain lookup,
no timer resumption, replacement by a new effect, native/Lua listing and deletion,
rollback, terrain edits, resize/load cleanup, and restart. Existing Rust timer
persistence remains intentional; this audit does not propose discarding running
Rust effect deadlines on restart. This ownership work is not yet implemented.


### Shared stored decoration ownership

The stored restoration model now covers FIRE, SMOKE and DECO with separate
ordinal collections and one record type, setter, validator and persistence path.
It preserves duplicate coordinates, lookup order and unrelated SQL payload fields.
Restored records remain unscheduled; neither saved budget fields nor object kind
create a timer. `hex` retains the saved visible marker, while `base_hex` resolves
restoration metadata only beneath FIRE/SMOKE. Ordinary terrain with leftover
metadata retains its visible identity. Explicit terrain replacement compares the
stored tile, so replacing a visible fire with its underlying terrain is not
mistaken for a no-op.

Native/Lua LIST and DELOBJ use typed selection identities to distinguish a stored
ordinal from an active overlay's tile index. Both can coexist without one masking
the other. Shared removal restores selected records' terrain. Shared effect
installation, used by operator/combat placement and autonomous spreading, removes
all prior restoration records at that coordinate and retains the chosen underlying
tile. Runtime deadlines continue to persist normally. Resize/load cleanup clears
stored records through the existing whole-map path, preserving visible terrain.
The terrain-save ownership guard now accepts stored FIRE/SMOKE rows.

New tests cover duplicates, source/tile index collisions, unscheduled reload,
lookup ordering, native/Lua deletion equality, late callback rollback, new-effect
replacement, timer expiry, explicit terrain replacement, resize and restart.
Three older fixtures used invalid or intentionally unowned smoke rows: the
building/mine tests now use valid restoration data and account for its ownership;
the unowned-object guard test uses an unknown object type. Payload-preservation
checks remain intact.

All 165 distinct focused checks pass across `target/imported-decorations-tests.log`
(non-surface targets), `target/imported-decorations-final.log`,
`target/imported-decorations-combat.log` (non-terrain targets), and
`target/imported-decorations-terrain-final.log`. The earlier failed expectations
in those logs are superseded by the named final runs. The separate full suite
in `target/update-links-full.log` is a snapshot from before this ownership change;
it is still running and has reported the live-duel pilot-recovery failure. The
fixture seeds player recovery dice but not empty-cockpit recovery dice that can
transfer to the player. That is a concrete candidate for its previously observed
intermittency, not yet a verified fix. Raw bit-table handling and broader
integration acceptance remain open; the goal remains active.
All-target Clippy with warnings denied passes in
`target/imported-decorations-clippy-final.log`; formatting, diff, Lua declaration
mirror and unchanged-reference checks pass.

### Deterministic duel recovery and lookup-cache invalidation

The completed `target/update-links-full.log` snapshot has 2,163 passes and one
failure across 258 result targets. Its only failure was the live duel's 300-second
pilot-recovery bound. The fixture seeded player recovery but left empty-cockpit
recovery random; a terminal hit can start that recovery and a subsequent cockpit
claim transfers it to the player. The fixture now seeds both streams before
combat and includes recovery state in a timeout diagnostic. Production recovery
rules and the test's time bound are unchanged. The duel passes three consecutive
runs (`target/duel-recovery-seeded.log`, `target/duel-recovery-repeat-2.log`, and
`target/duel-recovery-repeat-3.log`). Shared crew/recovery tests also pass.

Following the existing, stated derived-lookup design, saved mine/hangar cache
bytes no longer block terrain edits. A selective persistence step invalidates
only the changed map's cache after terrain/dimension, mine, or building-entrance
changes. Unrelated changes preserve untouched caches. Gameplay continues to use
typed definitions, with no duplicated mutable runtime cache. This does not add a
TBITS deletion command or claim a user answer to the earlier optional question.

Tests verify successful reload with nonzero cache bytes, preservation of unknown
object guards, per-map invalidation after definition changes, unchanged unrelated
caches, definition round trips, and full SQL rollback if cache invalidation fails
after object writes. The 35 distinct focused checks pass in
`target/duel-recovery-seeded.log`, `target/map-cache-invalidation.log` and
`target/map-cache-invalidation-final.log` (the final terrain run supersedes the
initial terrain run). All-target Clippy with warnings denied passes in
`target/map-cache-clippy.log`. Help describes cache invalidation; formatting,
diff and unchanged-reference checks pass.

A fresh full-suite run covering imported decorations, cache invalidation and the
seeded duel is starting in `target/map-cache-full.log`. Completion of the broader
integration audit is still unproven; the goal remains active.


### Shared mine order and native ADDMINE

Mine traversal now has a saved order independent of map-object record slots.
The optional owned `btech_mine_order` table stores that order; source imports
without explicit order use source ordinal order. Validation requires each live
record exactly once. Selective writes preserve auxiliary columns attached to
surviving records. Configuration insertion retains its established ordering;
operator and artillery insertion share `insert_minefield`, which prepends without
renumbering old records. Activation, command detonation, listing and blast cleanup
consume the shared order. Whole-map cleanup and owner destruction retain order
consistency; asset reload preserves it.

Wizard ADDMINE and `btech.map.add_mine` share one transactional action. All five
full type names are case-insensitive; extra defaults to zero, stored strength
clamps to signed 16-bit, and confirmation retains the signed 32-bit requested
strength. The actor owns the mine. Coordinate and numerical rejection leave state
unchanged; failed confirmation or late Lua errors restore both records and order.
No unit-specific launcher or placement logic was added. Artillery retains its
existing refusal to reinforce a coordinate already containing a mine.

Focused checks cover every type across all seven supported chassis, unchanged
unit state, exact native/Lua parity, original auxiliary columns, newest-first
order, removal, reload, corruption rejection, command diagnostics and rollback.
An artillery arrival/restart test verifies prepend behavior and no reinforcement.
The first order run exposed a configuration-insertion regression; the fix restores
the prior test's expected order without weakening the assertion.

The full run in `target/map-cache-full.log` was intentionally interrupted after
it picked up an intermediate mine-order executable overwritten by a focused build.
It is not a single-version acceptance result. Do not run other Cargo builds/tests
or edit compiled inputs while the replacement full suite is running.
All 178 distinct focused checks pass across `target/mine-order-acceptance.log`
(non-surface targets), `target/addmine-final.log` (surfaces),
`target/addmine-interfaces-final.log` (command/help interfaces), and
`target/addmine-records-verified.log` (the four final placement tests).
All-target Clippy with warnings denied passes in `target/addmine-clippy-final.log`;
formatting, diff, Lua declaration mirror and unchanged-reference checks pass.
A replacement full run starts in `target/addmine-full.log`; keep compiled inputs
and test executables unchanged until its process exits. The broader integration
goal remains active, with four map entry points and other operator/scenario
acceptance still open.


### Verified full baseline after ADDMINE

`cargo test --no-fail-fast` completed with exit status zero in
`target/addmine-full.log`: 2,171 tests passed, zero failed, across 260 result
targets. No other builds/tests ran and no compiled or runtime test inputs changed
during this run. It supersedes the interrupted/mixed-executable attempts for
whole-suite evidence. The live duel, imported decoration ownership, mine-cache
invalidation, mine traversal order and all existing interaction tests pass.

The full integration goal is still open. Continue with the four remaining map
entry points and their contracts in `btech-operator-audit.md`, then the broader
operator/Lua/configuration and supported-scenario acceptance review. A passing
suite does not license changing full-parity readiness markers yet.

## Map-side VIEW and tactical mine ordering

Native `VIEW X Y` and Lua `btech.map.view(actor, map, x, y)` now use one
transactional action. Wizard admission, signed coordinates, clamped centers,
saved player dimensions, terrain readiness, labelled output and ANSI preferences
are handled without requiring a cockpit or an observing unit. The terrain canvas,
hex glyphs, escaping and rectangle clipping are shared with cockpit rendering.
Map-only views omit unit/contact overlays and apply no scanner radius. Publication
failure and Lua callback failure restore staged output and world state.

The tactical mine overlay now follows the same explicit traversal order as mine
activation, detonation and operator listing. A newly inserted trigger field takes
precedence over an older explosive field at the same hex; inserting another
explosive field restores the marker without changing persistent record identities.

Focused acceptance passed 107 distinct tests across map views, MML, tactical scan,
vehicle maps, commands, access and help (`target/map-view-tests.log`). The final
map-view rerun (`target/map-view-final.log`) additionally checks ANSI-off output.
Coverage includes tiny maps, both column parities, all edges, extreme coordinates,
terrain/elevation cells, native/Lua equality, restart, publication rollback and
unchanged state across all seven supported chassis. The fixture uses explicit
bridge tiles so automatic road-to-bridge generation does not obscure rendering
expectations. These checks supplement the 2,171-test full baseline above; they
are not a new full-suite run. Map field entry points, FIXMAP and the remaining
operator/scenario audit are still open.
All-target Clippy with warnings denied passes (`target/map-view-clippy.log`).
Formatting, diff checks and the Lua type mirror comparison pass. The reference
repository remains unchanged.

## Named map field editing

Native `@SETMAP field value` and Lua `btech.map.set_field(actor, map, field, value)`
share one silent wizard action for all 13 writable map fields. Names match exactly
without regard to case. Read-only fields and unknown names reject. Building
integrity, light/visibility and wind delegate to existing controls; remaining
fields update a validated map record. Errors, including a later Lua callback
failure, restore world state and staged output. Names keep at most 29 UTF-8 bytes
without splitting characters or reloading terrain.

Generic gravity input preserves signed-byte clamping followed by unsigned
storage, independently of SETCOND's direct 0–255 input. Temperature and short
fields clamp to their storage widths. Typed invariants still reject invalid wind
bearings, negative wind speed or inconsistent construction integrity. Bitvectors
accept signed 32-bit integers or letters a–z/A–F, with `!` clearing the following
bit from a newly built value. Invalid input rejects instead of using undefined
shifts. Environment fields preserve existing flags, and edits neither advance
time nor schedule repairs.

Acceptance includes each writable field across all seven supported chassis,
native/Lua equality, persistence, silent success, invalid values, authority,
callback rollback, UTF-8 boundaries, gravity boundaries and bitvector boundaries.
The initial 41 distinct focused tests passed (`target/map-fields-tests.log` and
`target/map-fields-unit.log`); final gravity/help verification is in
`target/map-fields-final.log`. This supplements the full baseline above and does
not close @VIEWMAP, FIXMAP or the remaining operator/scenario audit.
The final map-field/help rerun and all-target Clippy with warnings denied passed
(`target/map-fields-clippy-final.log`). Formatting, diff checks and Lua mirror
comparison passed. The reference repository remains unchanged.

## FIXMAP with derived membership

Native `FIXMAP` and Lua `btech.map.check(actor, map)` use the same wizard action.
It checks the shared mixed-chassis membership order and validates world invariants
before publishing a checked-unit count and completion. The report returns map ID
and unit IDs in saved slot order. Failed checks publish no success text; partial
publication and later Lua callback failure discard staged output.

Rust derives the membership index from unit placement, so the reference's stale
pointer slots and independently mismatched index entries have no counterpart.
FIXMAP does not create a second index or erase valid placement to imitate pointer
repair. Duplicate slots, missing slots, incorrect object locations and mismatched
saved identities reject. Vehicle position/slot disagreement is rejected even
earlier by deserialization. Tests cover that boundary explicitly. Checks preserve
unit state, movement, timers, dice, slots and persistence across all seven
supported chassis, and also cover empty maps, authority and output rollback.

This implements the consistency-check purpose against the Rust state model.
It is not a recovery tool for an invalid database that startup cannot load.
@VIEWMAP and the remaining operator/scenario audit remain open.
Verification: all 36 focused tests passed (`target/map-check-tests.log`), as did
all-target Clippy with warnings denied (`target/map-check-clippy.log`). Formatting,
diff checks and Lua type mirror comparison passed. The reference tree is unchanged.
These checks supplement the full baseline above; no new full-suite claim is made.

## Parent-map metadata ownership

`StoredBattleMap.building_parent` now owns the existing `btech_maps.on_map`
column. New maps initialize it to zero. Imported values are preserved as metadata,
including references that are not current runtime maps; this value does not admit
travel or replace validated routes. Terrain replacement retains it. Link rebuilding
sets it on visited child maps, and the shared BUILD removal clears it alongside
return routes. LEAVE edits do not change it. Purging its referenced object clears
it without introducing a second routing relationship.

Acceptance imports a distinct saved parent value, edits return routes, saves and
restarts, rebuilds links, reloads terrain, tries a nonexistent BUILD deletion and
removes the actual BUILD record. The assertions distinguish authored configuration,
parent metadata and runtime return routes. Existing link rollback, cycles, depth,
terrain, resize and deletion coverage also passes. This resolves the metadata
prerequisite for @VIEWMAP; the field report itself remains open.
Verification: 91 focused tests passed across `target/map-parent-tests.log` and
`target/map-parent-persistence.log`. All-target Clippy with warnings denied passed
(`target/map-parent-clippy.log`), along with formatting and diff checks. The
reference tree remains unchanged. This is targeted evidence, not a new full run.

## Map field reports

Native `@VIEWMAP` and Lua `btech.map.fields(actor, map, arguments)` now publish
one shared wizard report. It contains all 18 catalogue entries in order, supports
a case-insensitive prefix filter and the leading 1/4 column selectors, and
uses two columns by default. Structured fields retain their complete names and
values even when narrow text layouts shorten labels. Object and asset names are
escaped for literal publication; the returned report text is unstyled text.

The report reads owned parent metadata independently of return routes. The C-only
historical allocation counter `firstfree` has no Rust equivalent: it displays
`n/a` and the structured field has no value, rather than substituting live unit
count. Gravity preserves the generic signed-byte view. Bitvectors show all 32
supported bits, including the sign bit, and use `-` for zero. The reference's
negative-bitvector omission is not reproduced.

Acceptance checks every default value and field position, prefix/layout variants,
unknown prefixes, native/Lua equality, literal markup characters, Unicode,
signed gravity and high-bit flags. It also covers all seven supported chassis,
unchanged simulation state, persistence/restart, authority and rollback after
partial publication or a later Lua callback failure. This closes the missing map
entry points; other operator and scenario requirements remain under audit.
The 35 focused checks passed (`target/map-field-report-tests.log` and
`target/map-field-report-unit.log`), as did all-target Clippy with warnings denied
(`target/map-field-report-clippy.log`), formatting, diff checks and Lua type mirror
comparison. A full run is being tracked separately in `target/map-operators-full.log`;
until it exits successfully, the earlier full baseline remains authoritative.

The first map-operator full run exited with a test failure in administration:
an alias test tried to define `view`, now occupied by the real VIEW command.
The fixture now uses `inspectroom` while retaining its alias-chain assertion.
All 26 administration/access/command checks pass
(`target/map-operator-alias-tests.log`). The stopped full run is diagnostic only;
a fresh uninterrupted run is tracked in `target/map-operators-full-verified.log`.


## Verified map-operator full baseline

`target/map-operators-full-verified.log` completed with exit zero: 2,186 tests
passed, zero failed, across 264 result targets including documentation. The exact
running process was followed to completion, without overlapping builds or changes
to its code/runtime inputs. The administration alias collision from the preceding
run was corrected before this run began. The earlier failed log is diagnostic
history, not part of the passing test count. The reference repository is unchanged.

The remaining work includes the debug command entry points, separate gunner
stations for supported parent units, and the wider configuration/Lua/scenario
acceptance audit. These are not closed by the passing full suite.

## Standalone weapon-setting commands

SETVRT and SETWBV now expose the existing runtime weapon controls directly.
They accept exactly two whitespace-separated arguments and a signed 32-bit value,
retain the shared range checks and exact case-insensitive canonical/manufacturer
weapon names, and reject switches. @btech retains its existing space/equal-sign
syntax. Both native forms share edit confirmation and mutation; Lua continues to
use those same typed mutation controls. No per-chassis setting logic was added.

The reference's very-long part lookup is exact hash lookup, not wildcard matching.
Both Rust commands require wizard authority, preserving the existing typed policy;
the source SETWBV help-marker omission would otherwise omit its privilege check.
The source's VRT wizard audit-log behavior remains part of the wider logging audit.

New acceptance compares standalone, @btech and Lua state/confirmation across
boundary values, rejects malformed syntax/names/switches and verifies denied
actors leave state unchanged. Existing setting tests retain cross-chassis firing,
active-countdown preservation, defensive activation, Battle Value experience and
runtime-reset-on-reload checks.
Verification: 35 focused tests passed (`target/debug-weapon-final.log`), along with
all-target Clippy with warnings denied (`target/debug-weapon-clippy.log`), formatting
and diff checks. The reference tree is unchanged. These checks supplement the
2,186-test baseline; the remaining debug, logging and scenario work stays open.

## Standalone skill-threshold command

SETXPLEVEL now accepts exactly a skill name and signed 32-bit threshold and
silently delegates to the shared runtime skill-threshold control. Wizard authority,
0–2147483647 bounds, canonical/alias resolution and non-skill rejection stay in
that control. @btech and Lua retain their existing interfaces. No XP computation
or character state logic is duplicated. Existing balances remain unchanged until
later awards; zero disables gains and overrides reset on database reload.

New acceptance compares native/Lua thresholds at zero, one, signed maximum and
the catalogue default; rejects malformed syntax, extra arguments, switches,
non-skills, overflow and unauthorized actors without changing state. Existing
character tests exercise actual awards, zero-threshold behavior, callback rollback,
selective persistence and restart. The reference's wizard audit-log behavior is
still part of the broader logging audit.
Verification: all 43 focused tests passed (`target/debug-threshold-tests.log`),
as did all-target Clippy with warnings denied (`target/debug-threshold-clippy.log`),
formatting and diff checks. The reference repository remains unchanged. The
2,186-test full baseline predates these standalone debug-command additions.

## Skill XP leaderboard

XPTOP and Lua `btech.character.xptop(actor, skill)` now share a read-only wizard
report. Canonical/alias skill resolution and the low-24-bit experience balance
come from the existing progression model. Wizard players and zero raw XP are
excluded; earned-level-only records remain eligible with zero balance. The first
10,000 eligible players in database order are counted, balances rank descending,
and at most sixteen entries are displayed. Ties use database order rather than
incidental swaps from the reference sorting loop. Totals use u64 and include
counted players outside the displayed sixteen. Zero totals yield zero percentages.

The structured report includes skill, counted players, total, entries and literal
text. Player names are escaped for publication. Queries do not award XP, rewrite
last-use timestamps or change simulation state. Later callback failure and partial
publication both discard staged report lines.

Acceptance covers concrete values/order/percentages, ties, levels versus balances,
wizard and zero-XP exclusion, unknown/non-skill rejection, native/Lua equality,
detached data, restart, zero totals and output rollback. A 10,001-player fixture
verifies the pre-ranking population limit and a 167,772,150,000-point total without
overflow. Remaining debug commands, audit logging, gunner stations and scenario
verification are still open.
Verification: all 46 focused tests passed (`target/xptop-tests.log`), as did
all-target Clippy with warnings denied (`target/xptop-clippy.log`), formatting,
diff checks and the Lua type mirror comparison. The reference tree is unchanged.
The 2,186-test full baseline predates these latest debug-command additions.

## Selected-map shutdown

SHUTDOWN with a numeric map argument now delegates to the existing wizard-only
bulk map clear action. The first argument is a signed 64-bit database number;
trailing arguments are ignored. Argument-free SHUTDOWN retains its existing
cockpit control. The selected-map form does not depend on the operator's current
location and does not change that location.

The existing lifecycle matrix now compares remote SHUTDOWN, local CLEARMECHS and
Lua clearing across all supported chassis and applicable off, starting, running,
moving and airborne states. They produce equal BattleTech state through shared
shutdown consequences, towing cleanup and membership detachment. Tests also
verify invalid map numbers, overflow and unauthorized actors leave state unchanged,
and bare shutdown leaves both source and target on their map while stopping only
the source. Existing rollback/restart and power/towing checks remain in acceptance.

All 79 focused tests passed (`target/debug-map-shutdown-tests.log`), as did
all-target Clippy with warnings denied (`target/debug-map-shutdown-clippy.log`),
formatting and diff checks. The reference tree is unchanged. The full baseline
predates this change; remaining debug reports/checkpoint, audit logging, gunner
stations and broader scenario requirements remain open.

## Explicit SQLite checkpoint requests

SAVEDB and Lua `btech.database.save(actor)` now stage one coalesced forced-save
effect in the ordinary world transaction. Wizard authority is checked at admission.
The host bypasses its unchanged-world persistence shortcut when this effect is
present, uses the existing transactional writer, then publishes the queued SQLite
checkpoint confirmation. Failure uses the common persistence error response and
discards the success message. No second writer or detached database operation was
introduced. The Lua boolean means queued, not already persisted.

The flag participates in savepoints, rollback and commit cleanup. Acceptance
checks repeated requests, restoration of both requested/unrequested savepoints,
commit cleanup, rollback, Lua callback abort and authority. A live TCP server test
performs a checkpoint, holds a SQLite write lock, verifies failure without success,
releases the lock and verifies a successful retry with unchanged BattleTech state.

The Lua declaration file now exposes the database table in the root declaration
and returns that root after all declarations. A compile check guards the previously
misplaced return statement. The fixture copy remains identical. Remaining debug
reports, audit logging, gunner stations and scenario requirements remain open.
Verification: 49 distinct focused checks passed across `target/savedb-tests.log`,
`target/savedb-effects-tests.log` and `target/savedb-server-tests.log`. All-target
Clippy with warnings denied passed (`target/savedb-clippy.log`), along with
formatting, diff checks and Lua mirror comparison. The reference tree is unchanged.
The 2,186-test full baseline predates the recent debug-command additions.

## Shared catalogue form inspection

LISTFORMS and Lua `btech.inventory.forms(actor)` now expose short, long and
very-long names from the existing inventory selection index. The report includes
all 575 base stock identities and the supported manufacturer variants, independent
of inventory quantities. Sorting follows short names with deterministic brand/part
identity ties; existing transfer selection order and collision priority are retained.

The native wizard command ignores trailing arguments and uses paced report delivery.
Acceptance compares every native row with the shared catalogue, checks Lua names
and detached results, authority, rejected switches and unchanged world state. A live
TCP test receives every row of the full catalogue under default output limits.
Both focused tests passed in `target/listforms-tests.log`. The uninterrupted full
suite passed all 2,196 tests (`target/listforms-full.log`). EVENTSTATS, MEMSTATS,
semantic audit logging,
gunner stations and broader scenario requirements remain open.

All-target Clippy with warnings denied passed (`target/listforms-clippy.log`),
as did formatting, diff checks and the mirrored Lua declaration comparison.
The reference tree remains unchanged.

## Transactional operator audit records

VRT and skill-threshold operator edits now share actions across standalone commands,
@btech and Lua. These actions retain the typed controls and their authority/range
checks, and stage canonical WIZ/CHANGE records when wizard logging is enabled.
SETWBV remains without the semantic record, matching the reference. Low-level
World mutators remain available for construction and embedding without logging.

Categorized records use the existing effect savepoints, aggregate output limits,
rollback, reload inheritance and post-commit flush. Audit admission occurs before
the candidate world replaces the live world, so an exhausted output budget cannot
partially change a setting. Settings retain their existing runtime-only lifetime;
the diagnostic reports a committed runtime change, not a persisted override.

Acceptance covers all three operator entry paths, canonical names, no-op successful
edits, denied/invalid requests, callback rollback, logging disabled, record limits,
savepoints and inheritance. A server test pairs an edit with a persisted object
change, rejects that database update, and checks that the setting and record are
rolled back before retrying successfully. The earlier 2,196-test full baseline
predates this addition. Runtime statistics, gunner stations and broader scenario
requirements remain open.

Verification: 47 focused checks passed across `target/operator-audit-tests.log`
(28), `target/operator-audit-server-tests.log` (17), and
`target/operator-audit-effects-tests.log` (2). All-target Clippy with warnings
denied passed (`target/operator-audit-clippy.log`), along with formatting and diff
checks. The reference tree is unchanged.

## Separate gunner-station ownership

Rust now owns the `btech_turrets` station fields, preserving the independent target,
coordinates, lock modes and arcs. These fields are loaded regardless of whether
the parent is supported; actions require an available constructed supported parent.
Selective writes preserve auxiliary TIC rows and parent-link arrays. Object purge
removes station ownership or clears destroyed parent/gunner/target references in
both the in-memory projection and existing relational maintenance.

Wizard Lua registration authors a new station on an unregistered thing through
`btech.gunner.register(actor, station, parent, arcs)`. Native INITIALIZE/DEINITIALIZE
and Lua counterparts share occupant checks, connected-gunner takeover rules and
atomic notifications. Detached inspection exposes saved fields without granting
control. Admission returns an explicit station/parent/gunner/arc context and rejects
simultaneous parent piloting; it never changes the parent's pilot or targeting.

Acceptance covers all seven supported chassis, repeated initialization, blocked
takeover, disconnected and departed gunners, release, wrong actors, callback and
publication rollback, authority and unsupported parents. Restart checks retain
assignment and aiming fields; targeted SQL fixtures retain signed coordinates and
flags as well as an unrelated TIC row. Parent/station deletion exercises ordinary
database maintenance.

This establishes station ownership, not functioning station combat. Next work must
thread the station context through shared targeting, arc checks, skill selection,
firing and report actions. Native field controls, lock progression, cross-station
isolation and complete combat/restart scenarios remain open. Do not temporarily
replace the parent pilot or duplicate launcher logic. The 2,196-test full baseline
predates this and the operator-audit-record change.

Verification: 101 distinct focused checks passed (`target/gunner-stations-tests.log`).
The three station scenarios passed again after tightening a test snapshot's borrow
lifetime (`target/gunner-stations-final-tests.log`). All-target Clippy with warnings
denied passed (`target/gunner-stations-clippy.log`), as did formatting, diff checks
and the Lua declaration mirror comparison. The reference tree is unchanged.

## Independent gunner target selection

Native LOCK and Lua `btech.gunner.lock` / `lock_hex` reuse the ordinary target
selector with separate selection ownership and parent sensor identity. Present
registered gunners pass shared consciousness/blindness and running-parent gates.
Contact eligibility, map coordinate checks and lock-purpose parsing use existing
rules; parent pilot, target and artillery adjustment remain independent. Saved
coordinate locks retain map elevation and the game-directory lock-mode bits.

Reference evidence in `combat/mech_combat.c` and `core/context.c` establishes that
nonzero station arc overrides bypass the lock event. Those stations lock immediately.
Zero-mask stations use the common eight-second countdown and completion messages.
Their remaining time lives in an optional station-owned timer table, created only
by an explicit settling write. Progress uses the shared lock heartbeat, including
idle parents, and surviving timers retain their value across restart.

Shared contact loss clears affected station targets; battlefield removal clears
outgoing/incoming station selections. Relational maintenance removes pending clocks
when a station, parent or target is purged. A rejected timer update leaves the
previous durable value available for retry.

Acceptance compares two independent stations and their parent across all seven
supported chassis; covers native/Lua selection, all coordinate purposes, invalid
admission, callback rollback, restart mid-countdown, completion recipients, explicit
arc immediate locks, map removal and database failure/retry. A live TCP case verifies
completion while both parent and target are shut down and optical scanning is idle.
Station artillery firing is covered below. Broader report forwarding, native field
controls and wider sensor/lifecycle interactions still require acceptance work.
The integration goal remains open; the 2,196-test full baseline predates these
station changes and the semantic operator audit records.

Verification: all 34 focused checks passed in `target/gunner-lock-final-tests.log`.
All-target Clippy with warnings denied passed (`target/gunner-lock-clippy.log`),
along with formatting, diff checks and the Lua declaration mirror comparison.
The reference tree is unchanged.

## Explicit operator skill calculation

Captured gunner contexts now revalidate station, parent, assigned gunner and arc
identity before resolving skills. They use the shared installed-weapon lookup and
chassis/weapon-family policy with the gunner as an explicit operator. Parent pilot
identity, piloting skill, target state and character records are never temporarily
replaced. Disconnected station crew uses the ordinary gunnery fallback six and
the dedicated artillery fallback eight. Stale or forged contexts are rejected.
Lua exposes read-only `btech.gunner.gunnery(station, gunner, weapon)` and
`artillery_gunnery(station, gunner)` inspection; these queries do not authorize a shot.

Reference `unit/mech_identity.c::find_gunnery_skill_name` selects
Gunnery-Aerospace for VTOLs when extended gunnery is disabled. Rust incorrectly
selected Gunnery-Conventional for every vehicle. One shared skill selector now
serves hit-target arithmetic and classic/Battle Value XP selection, correcting VTOL
behavior while preserving ground-vehicle and Mech policy. Dedicated artillery
queries retain their separate skill/fallback regardless of extended gunnery.

Acceptance varies parent and gunner attributes and skills across seven chassis
and three weapon families, with both skill policies, earned levels, disconnected
crew, invalid weapon indices and invalidated contexts. The VTOL cockpit check
verifies Aerospace selection and restart without changing piloting; an actual XP
award verifies that the same skill receives experience.

Direct-fire admission now carries station ownership through target decoding,
launch, aim, XP eligibility and feedback. Station artillery shares the same
operator source, as detailed below. The 2,196-test baseline predates these
operator and station changes.

Verification: all 39 focused checks passed (`target/gunner-skills-final-tests.log`).
All-target Clippy with warnings denied passed (`target/gunner-skills-clippy.log`),
as did formatting, diff checks and the Lua declaration mirror comparison. The
reference tree is unchanged.

## Gunner equipment reports

Native WEAPONSPECS, WEAPONSTATUS and CRITSTATUS now resolve claimed stations to
parent equipment through the existing report admission helper. Their formatting,
section parsing and equipment rules remain shared across all seven supported
chassis. Station admission verifies the assigned gunner and parent; ordinary
critical inspection retains pilot authority. Conscious observation checks the
gunner's recovery and the physical parent's blindness. Specifications remain
available without conscious observation, and weapon condition still requires a
placed parent. This does not grant movement or firing authority.

Two station regression tests cover all seven chassis, exact pilot/gunner output,
release and uninitialized denial, ordinary passenger restrictions, unchanged
BattleTech state, and both observation guards. The 20 focused report, station
and help checks pass in `target/gunner-reports-final-tests.log`. Terrain/artillery
firing, other cockpit report forwarding and dedicated Lua report conveniences
remain open. The last full-suite baseline still predates gunner integration.

All-target Clippy with warnings denied also passed
(`target/gunner-reports-clippy.log`), as did formatting and diff checks. The
reference tree remains unchanged.

## Independent gunner target and aim source

Target decoding and conventional aim now carry physical equipment identity and
selection ownership separately. Cockpit adapters use the same unit for both;
registered stations supply their parent and station ID. The shared resolver
handles selected units, occupied and empty coordinates, self-cooling and observer
links. Station previews do not fall back to a parent's lock. A station's unit
lock overrides the parent's observer link independently of the cockpit lock.
Hex purpose and ordinary settling penalties likewise come from the station.

`BattleGunnerContext::aim` and `btech.gunner.aim(station, gunner, weapon, target)`
expose a detached conventional preview using the registered gunner's skill and
shared Mech/vehicle/VTOL calculations. The optional target accepts a unit ID,
coordinates, or the station selection. Preview sensor dice are cloned. This is
read-only aim inspection: it does not authorize launch, test station arc-mask
eligibility, consume ammunition or implement native station SIGHT. Dedicated
artillery preview remains separate; live station artillery is covered below.

Direct firing reuses this target source through combat-specific operator
admission, shared reservation/launch, aim, safety checks and XP. Ordinary pilot
controls remain separate. Native station SIGHT still requires that same explicit owner/parent/operator
distinction; artillery launch now uses it.

Verification: 57 focused checks passed (`target/gunner-target-source-tests.log`).
Expanded mixed LRM/MML and empty-coordinate observer coverage passed in the final
nine-check run (`target/gunner-target-source-final-tests.log`). The all-chassis
station tests cover empty/settling/settled selection, parent-lock independence,
gunner skill, coordinate modes, detached output, unchanged dice and state,
restart/disconnected fallback/reconnection, and access revocation. The most
recent full-suite baseline still predates gunner integration.
All-target Clippy with warnings denied passed
(`target/gunner-target-source-clippy.log`), together with formatting, diff checks
and matching Lua declarations. The reference tree remains unchanged.


## Live station direct fire

Native WEAPONS uses the shared station report lookup for weapon numbers, supply
and recycle. Native FIRE and `btech.gunner.fire(station, gunner, weapon, target)` resolve
unit-target shots through shared Mech, vehicle and VTOL combat. Selected units,
explicit units and occupied coordinates use station-owned locks and gunner skills.
The parent retains its pilot and selection. Mechanical checks, launch rolls,
ammunition, heat, recycle, damage and casualty publication remain in the existing
combat implementations; no second launcher implementation or temporary pilot
substitution was introduced.

`combat_operator` revalidates the actor's claimed station at combat boundaries.
Running placement and weapons-hold checks are shared with cockpit admission.
Weapon reservation and launch accept this operator while ordinary radio, movement
and shutdown still require the parent pilot. Mech-only entry points retain their
anatomy guard. Gunner consciousness and parent blindness are checked independently.
Explicit arc masks restrict available directions; zero retains ordinary mount
arcs and settling costs. Stable stealth locks are checked against the station's
selection. Shooter injury/recoil toughness still belongs to the physical pilot.

Classic and Battle Value XP eligibility accept the current connected gunner;
awards use the shared chassis/family skill policy. Live in-character damage tests
verify awards to the gunner without changing the pilot's skills. Parent cockpit
feedback is also staged for the station audience. Publication failure restores
world changes and all staged notices.

Station artillery now uses the same source/operator distinction and shared
launcher, as detailed below. Native SIGHT, wider reports, field controls,
additional casualty/observer scenarios and the overall integration audit remain
open. The last full-suite baseline predates these changes.

Verification: 438 regression checks passed in `target/gunner-fire-regression-tests.log`.
The final 21-check run (eight station, six equipment-report and seven help checks)
passed in `target/gunner-fire-final-tests.log`. These include four weapon families on seven
chassis, actual target damage, native/Lua/pilot result comparisons, independent
skills/locks, restart, role/hold/health guards, heading-based arc masks, XP and
transaction/publication rollback. Admission sharing was subsequently verified
with 13 station, TIC and weapon-power checks in `target/gunner-fire-admission-tests.log`.
All-target Clippy with warnings denied passed (`target/gunner-fire-clippy.log`),
as did formatting, diff checks and matching Lua declarations. The reference tree
remains unchanged. This is unit/terrain fire delivery, not overall integration completion.


## Station terrain firing

Native and Lua station FIRE now support empty coordinates and terrain-purpose
locks through the same `hex_shot` and `coordinate_launch` paths as cockpit fire.
The station supplies the operator skill, target purpose and observer override;
physical equipment supplies ammunition, heat, recycle and terrain/surface/building
effects. Ignition, clearing, building, hex and unit-at-hex purposes retain their
existing rules. No additional launcher or damage implementation was introduced.

Unit and coordinate attacks share station arc-mask checks, with coordinates
measured against their map center. Zero masks retain ordinary mounting/indirect
rules. Source injury and recoil toughness comes from the physical pilot, while
consciousness and gunnery use the station operator. The existing host transaction
publishes terrain consequences and forwards shooter feedback to the station.

Tests compare two weapon families and all five purposes across all seven chassis
against cockpit results, including native/Lua equivalence, independent parent
selection, callback rollback and committed-state restart. Mixed Mech/vehicle LRM
and MML live observer shots verify that a parent unit lock does not replace the
station's empty-coordinate observer target. Invalid station arcs reject without
expenditure or output. The 162-check run in `target/gunner-terrain-final-tests.log`
passed, including surface/building, terrain, target, artillery, observer and help
regressions. The initial 23-check run also passed (`target/gunner-terrain-tests.log`).

Station artillery preparation and queued impact ownership are now implemented
with explicit station selection, skill and correction, as detailed below.
Native station SIGHT, broader controls/reports and final integration scenarios
also remain open. The full-suite baseline still predates gunner integration.
All-target Clippy with warnings denied passed (`target/gunner-terrain-clippy.log`),
along with formatting, diff checks and matching Lua declarations. The reference
tree is unchanged.


## Station artillery launch and correction

Station FIRE and Lua `btech.gunner.fire` now use the common artillery preparation,
coordinate expenditure and delayed-impact queue. The operator supplies selection,
artillery skill, firing arcs and correction; the physical parent supplies equipment,
ammunition, heat, position and pilot toughness. Conventional `btech.gunner.aim`
remains a separate read-only preview and does not preview artillery.

Queued rounds retain the physical shooter and optional historical station owner.
A shell survives restart and station removal. An observed miss updates only the
matching station’s correction, never the parent or another station. Retargeting,
contact removal and observer-link changes clear affected corrections; changing
only the parent’s target preserves an independently selected station correction.
An optional station-owned SQL table stores nonzero correction values within the
world transaction. Failed writes preserve the previous durable state, and database
maintenance removes corrections belonging to purged stations, parents or targets.

Acceptance covers biped and quad Mechs, tracked, wheeled, hover and stationary
vehicles, and VTOLs using the same firing adapter. Cases include native/Lua launch
agreement, independent parent/station selections, gunner artillery skill, correction
applied to the next shot, callback rollback, failed SQL update, restart midway
through flight and station deletion before arrival. Observer retargeting and
parent datalink changes also exercise correction invalidation.

Station SIGHT is covered below. Broader reports/controls and the final integration
audit remain open. This completes station artillery, not the entire integration goal.

Verification: all 2,230 tests passed across 274 result targets in
`target/gunner-artillery-full-tests.log`. This is the current full-suite baseline
and includes station ownership, targeting, skills, direct/terrain/artillery fire
and the new observer-invalidation checks. The earlier focused artillery run
passed 48 tests (`target/gunner-artillery-final-tests.log`).
All-target Clippy with warnings denied passed (`target/gunner-artillery-clippy.log`),
as did formatting, diff checks and the Lua declaration mirror comparison.
The reference tree remains unchanged.


## Station SIGHT

Native `sight` and Lua `btech.gunner.sight` now share the cockpit action, carrying
the operator's selection, skill and arc mask into unit, terrain and artillery
aim. A separate running-control admission is shared with firing; firing adds the
weapons-hold gate, while sighting retains its existing permission on held,
recycling and empty weapons. No pilot or parent targeting substitution is used.
The same explicit-source functions now serve all sight and fire callers; unused
cockpit-only forwarding adapters were removed.

The operation consumes its ordinary dice and publishes to the station. It does
not reveal cover, change heat, consume ammunition, launch artillery or alter
correction. Native/Lua and pilot comparisons span all seven chassis and four
conventional weapon families. Independent station selections, skills, rejected
arcs, released controls, observed LRM/MML unit and empty-coordinate targets,
artillery, callback/publication rollback and restart are checked. Player help and
mirrored Lua declarations include the command.

Broader station reports, field controls and final integration acceptance remain
unfinished. The 2,230-test full baseline predates this SIGHT change.

Verification: all 69 focused tests passed in `target/gunner-sight-final-tests.log`,
including gunner fire/sight/aim/targeting, ordinary sight/aim, artillery, observers,
weapon power, TIC and help. The initial 34-test run also passed
(`target/gunner-sight-tests.log`). All-target Clippy with warnings denied passed
(`target/gunner-sight-clippy.log`), as did formatting, diff checks and matching Lua
declarations. The reference tree is unchanged. The full integration goal remains open.


## Station navigation measurements

BEARING, RANGE, VECTOR, ETA and FINDCENTER now accept registered station gunners.
The matching `btech.gunner` methods forward through the existing unit measurement
bindings after station admission. Read-only report access supplies separate
physical-unit and selection-owner identities; ordinary measurement passengers
retain their access, while FINDCENTER still requires pilot or gunner control.
Movement and mutable display controls are not broadened.

Shared endpoint decoding uses station selections and parent sensors. RANGE uses
parent altitude for dark-map masking. ETA and FINDCENTER now use the common unit
view across supported Mechs, ground vehicles and VTOLs instead of indexing Mech
storage. ETA uses actual speed magnitude, accepts its existing explicit coordinate
grammar and requires an ordinary hex for a default. Its notices go to the owning
cockpit or station, with publication rollback. No measurement mutates combat state.

Acceptance compares native/Lua and pilot/gunner reports across all seven chassis,
including forward/reverse/stationary ETA, independent defaults, unit contacts,
darkness, restart, release, wrong actors, blindness, unconsciousness and parent
removal. A capped-output case verifies ETA rollback. The earlier station SIGHT
arc test was corrected to use an in-bounds coordinate and check the arc error;
it had previously rejected its out-of-bounds test coordinate before checking arcs.

Tactical/navigation map rendering, contacts, scan/report/status forwarding, field
controls and final integration acceptance remain open. The 2,230-test full-suite
baseline predates station SIGHT and these measurement changes.

Verification: the final 83-test navigation/report/scan/map/help run passed
(`target/gunner-navigation-final-tests.log`). The earlier 84-test run also passed
(`target/gunner-navigation-tests.log`), including all 12 gunner fire/SIGHT checks
with the corrected arc assertion. All-target Clippy with warnings denied passed
(`target/gunner-navigation-clippy.log`), along with formatting, diff checks and
the Lua declaration mirror comparison. The reference tree remains unchanged.


## Station battlefield displays

TACTICAL, LRSMAP and NAVIGATE now resolve registered station ownership before
using the common viewport and renderers. Center parsing receives the physical
parent after admission, so contact identifiers resolve against parent sensors.
Terrain, dark-map masking, mines, landing/cliff overlays and own-unit markers all
use that same physical identity. ANSI and saved dimensions remain viewer-owned.
Default centering follows the parent and does not replace either target lock.

`btech.gunner.tactical`, `lrsmap` and `navigate` reuse installed unit bindings.
The station facade forwards arguments without changing the unit grammar. Shared
operator resolution was factored out of running/fire admission; map centering
retains its own power and hardware ordering. Local navigation keeps the current
hex exception for failed scanner hardware. Displays leave world state, dice and
outbox unchanged; they grant no pilot controls.

Acceptance compares native/Lua and parent/station output across seven chassis,
fifteen renderer/mode combinations, own/contact/projected centers and both ANSI
settings. Separate cases cover gunner dimensions, dark maps, disabled scanner
hardware, blindness, release, callback rollback and restart. SCAN, REPORT, STATUS,
CONTACTS, field controls and final integration acceptance remain open. The last
full-suite baseline predates these station display changes.

Verification: all 77 map/navigation/scan tests passed (`target/gunner-maps-tests.log`),
followed by 44 fire/station/SIGHT/artillery/help regressions
(`target/gunner-maps-regression-tests.log`). All-target Clippy with warnings denied
passed (`target/gunner-maps-clippy.log`), along with formatting, diff checks and
matching Lua declarations. The reference tree remains unchanged.


## Station SCAN and REPORT

Native SCAN and REPORT now resolve the gunner's selected unit or coordinate while
using the physical parent's sensors, map membership, range and observer role.
Unit/occupant reports share ordinary disclosure and visibility checks. Scan
warnings identify the physical scanner to its target; REPORT remains silent.
Selected Building/Hex and UnitAtHex/Ignite/Clear modes retain the common dispatch
and do not change either lock or countdown.

Building and mine scans normalize physical scanner identity before their shared
perception/dice paths. Cached scanner perception remains parent-owned; eligible
experience is awarded to the acting gunner. Cockpit results are routed to the
station, while failed mine recognition remains private to the actor. Both phases,
dice, experience diagnostics and notices share the existing rollback boundaries.
`btech.gunner.scan`, `report`, `scan_hex`, `scan_building`, `scan_terrain` and
`scan_selected` reuse the unit bindings through registered-station guards.

Acceptance covers all seven chassis as scanner and recipient, native/Lua output,
unit and coordinate selections, disclosure, physical warning identity, restart,
concealed structures, mine recognition and gunner experience. Guards cover invalid
actors, blindness, shutdown and station release. Publication failure is forced
after a building notice, during the mine phase, to verify complete rollback.
STATUS, CONTACTS, field controls and final integration acceptance remain open.

Verification: all 94 focused scan/report/navigation/help tests passed
(`target/gunner-scan-final-tests.log`). The initial 77-test scan run passed
(`target/gunner-scan-tests.log`); seven help checks passed again after the help
and Lua declarations were updated (`target/gunner-scan-help-tests.log`).
All-target Clippy with warnings denied passed (`target/gunner-scan-clippy.log`),
as did formatting, diff checks and the declaration mirror comparison.
The reference tree remains unchanged; the full integration goal remains open.

## Station STATUS

Native STATUS and guarded `btech.gunner.status(station, gunner, options)` now
use the shared Mech/vehicle renderers. Physical condition, weapons, motion,
sensors and observer facts come from the parent. Selected unit/hex and settling
time come from the station without temporarily modifying the parent. The global
`btech.unit.status` inspector retains its existing read-only signature.

Acceptance covers seven chassis, native/Lua equality, all display sections,
independent selections, parent-selection preservation, health guards, station
release, passenger access, shutdown and persistence. MML acceptance was rerun:
all five integration tests passed for functioning SRM/LRM combat through shared
launcher rules. CONTACTS, field controls and final integration acceptance remain
open; this slice does not mark the overall integration complete.

Verification: 2,243 distinct tests passed across the initial full-suite run
(`target/gunner-status-full-tests.log`, 502 passing tests before the STATUS
fixture failure), corrected station tests (`target/gunner-status-final-tests.log`,
3 passed), and remaining integration targets
(`target/gunner-status-remaining-tests.log`, 1,738 passed). The fixture now uses
real shutdown rather than directly creating an invalid power/lock combination.
Documentation tests passed (`target/gunner-status-doc-tests.log`). Formatting,
diff checks and mirrored Lua declarations passed; the reference tree is unchanged.
All-target Clippy with warnings denied passed (`target/gunner-status-clippy.log`).

## Station CONTACTS

Native CONTACTS now uses a shared occupant report service. Parent sensors, acquired
visibility, brief mode, sort order and physical contact facts remain common across
Mechs, ground vehicles and VTOLs. Target-only filtering and selected-row highlighting
use the station's selection. Saved contact preferences belong to the viewing gunner.
`btech.gunner.contacts(station, gunner, options)` exposes this same formatted report
with callback and station guards; the existing unit contact data query is unchanged.

Building contacts capture physical parent and original selection owner separately.
Identification locks use the gunner as enactor/subject and parent as cause. Before
and after callbacks, admission checks both original owner and parent. Moving the
actor into the parent cockpit, reassigning the station, or removing the station
fails the report and rolls back callback world/effect changes. Invisible structures
never execute identification locks; denied hidden structures remain undisclosed.

Acceptance covers seven chassis, four brief modes, native/Lua equality, saved and
per-call preferences, independent target inclusion, restart, stale/unavailable
contacts, passenger access, released stations, blindness/unconsciousness, lock
identity and native/Lua rollback after ownership changes. All 90 focused tests
passed (`target/gunner-contacts-final-tests.log`). The initial 72 scan/report tests
also passed (`target/gunner-contacts-initial.log`). The preceding 2,243-test full
suite predates this CONTACTS change. Station field controls and final integration
acceptance remain open; the overall integration is not yet complete.
All-target Clippy with warnings denied passed (`target/gunner-contacts-clippy.log`),
as did formatting, diff checks and the Lua declaration mirror comparison.
The reference tree remains unchanged.

## Station wizard fields

`@SETTURRET field value` and `@VIEWTURRET [1|4][prefix]` now operate on the
wizard's occupied station. Guarded Lua `btech.gunner.set_field` and `view_fields`
allow explicit station administration. Field names are case-insensitive and exact
for edits; inspection retains catalogue order and prefix matching. Maps and
stations share one column-layout renderer and option parser.

The eight fields are arcs, parent, gunner, target, targx, targy, targz and lockmode.
References parse as decimal signed 64-bit values; arcs/lockmode parse as signed
32-bit integers, and coordinates clamp parsed 32-bit integers to signed 16-bit
limits. Coordinates own separate storage: the reference catalogue incorrectly
points all three at the target reference. Rust edits coordinates without altering
the selected target ID. Field edits reset stale settling/correction progress.
Deferred parent references remain inspectable and durable, but operator admission
continues to require an available supported parent and assigned occupant.

Seven-chassis tests compare native/Lua edits and reports, all fields and column
modes, exact parsing and coordinate clamping, parent-state preservation, denied
access, callback rollback, restart, and partial-report output failure. Map-field
regressions cover the extracted shared layout. Runtime EVENTSTATS/MEMSTATS and the
remaining cross-command/scenario acceptance audit are still open; the integration
goal remains active.

Verification: all 20 focused field/map/contact/access/help tests passed
(`target/gunner-fields-final-tests.log`). All-target Clippy with warnings denied
passed (`target/gunner-fields-clippy.log`), along with formatting, diff checks and
matching Lua declarations. The reference tree remains unchanged. The prior full
suite remains the baseline pending final integration acceptance.

## Reserved station TICs and runtime diagnostics

The previous station TIC audit finding was corrected against actual handlers:
the five reference station TIC functions are empty. Native ADDTIC/DELTIC/CLEARTIC/
LISTTIC/FIRETIC now return silently for registered stations, preserving existing
behavior without granting control over the pilot's groups. Shared cockpit TIC
logic remains unchanged; tests cover all seven chassis, valid/invalid arguments,
initialized/uninitialized stations, state/dice preservation and parent authority.

Wizard EVENTSTATS and MEMSTATS [LONG] now have native and guarded Lua
`btech.runtime.stats(actor)` access. EVENTSTATS shares the server's actual one-second
simulation-work predicate and reports scanner observers, queued artillery,
settling station locks and reactor startup grace. MEMSTATS reports live records,
inline root/map/unit/station sizes excluding heap storage, and exact compact JSON
state size through a counting writer. LONG reports registration kinds. Allocator
totals are explicitly unavailable; representation sizes are not heap estimates.
The shared scheduler predicate preserves existing tick conditions and reuses the
server's already collected scanner list.

Acceptance compares typed/native/Lua values across seven chassis and restarts,
checks exact encoding size, permission denial and read-only behavior, and verifies
that the empty-world startup grace expires into an idle scheduler. Existing TIC
and server power/tick-save rollback tests cover the affected shared paths. The
remaining integration goal still requires the broader command/scenario audit.

Verification: 26 distinct focused tests passed: 5 access, 1 station TIC and 10
power/tick tests in `target/btech-runtime-stats-tests.log`, plus the corrected
runtime snapshot test, 2 cockpit TIC tests and 7 help tests in
`target/btech-runtime-stats-final-tests.log`. The initial empty-world assumption
was corrected to account for the real 31-second reactor startup grace; the test
now verifies both active grace and subsequent idle state. All-target Clippy with
warnings denied passed (`target/btech-runtime-stats-clippy.log`), as did formatting,
diff checks and Lua declaration mirroring. The reference tree is unchanged.
The last full-suite baseline predates this scheduler extraction.

## HEAT alias and SETTEAM

HEAT now aliases FLAMERHEAT in the native registry. Both use identical selection,
readiness, duplicate handling and transaction logic across supported chassis.
Wizard SETTEAM and `btech.unit.set_team(actor, unit, team)` use one placed-unit
action. Negative signed 32-bit input becomes zero; team changes reuse the shared
sensor signature setter, preserving hidden/illuminated facts and clearing the
unit's C3/C3i assignment. Validation and notification failures roll back world
and effects. Tests cover seven chassis, native/Lua equality, integer bounds,
permissions, callback rollback, signature preservation and restart.

The operator audit now explicitly tracks remaining wizard scenario/field commands
rather than implying runtime statistics completed the catalogue comparison.

Verification: 38 command/access/C3i/vehicle-network tests passed
(`target/btech-scenario-command-tests.log`) and all 7 help tests passed
(`target/btech-scenario-help-tests.log`). All-target Clippy with warnings denied
passed (`target/btech-scenario-clippy.log`), as did formatting, diff checks and
matching Lua declarations. The reference tree remains unchanged. Full integration
acceptance remains open, including the wizard commands named in the operator audit.

## Wizard LOS emotes

Native @LOSEMIT and `btech.unit.losemit(actor, unit, message)` now use the shared
observer-message selector. Source state must be available and placed, but source
power and pilot assignment do not gate wizard emotes. The actor must be conscious.
Running observers must currently see the source; its own cockpit is excluded.
The existing identification and leading-apostrophe rules format each emote, and
publication escapes literal text. Empty messages retain ordinary emote formatting.
The actor receives a private `Broadcast done.` confirmation. All observer messages
and confirmation share one rollback boundary; no contacts, dice or world state
are changed by the action.

Seven-chassis acceptance covers native/Lua delivery, source exclusion, literal
markup, possessive spacing, permissions, callback rollback, restart, stopped
sources and blinded observers. A bounded-outbox test preloads one private line,
then forces failure after the observer line but before confirmation, proving that
only preexisting output remains. Existing visibility and map-broadcast tests
cover the shared audience and publication paths. Other wizard scenario/field
commands and full integration acceptance remain open.

Verification: 20 distinct focused tests passed across access
(`target/losemit-access-tests.log`), visibility/map-broadcast
(`target/losemit-tests.log`), and final emote/help acceptance
(`target/losemit-final-tests.log`). All-target Clippy with warnings denied passed
(`target/losemit-clippy.log`), along with formatting, diff checks and matching Lua
declarations. The reference tree remains unchanged; full integration acceptance
is not yet complete.

## Wizard located damage

@DAMAGESECTION and `btech.unit.damage_section(actor, unit, section, damage, rear,
critical)` now apply a located hit through the existing impact/vehicle armor,
critical, crew and evacuation paths. Native damage is a signed integer in 1–1000;
flags accept signed integers with nonzero meaning true. Lua uses booleans.
Section parsing follows chassis anatomy. Vehicle front hits with rear set redirect
to rear armor and retain the independent rear diagnostic roll. Wizard hits retain
self attribution; physical damage, notices and casualties share existing action
rollback. Source power, pilot assignment and map placement are not prerequisites.

The shared vehicle armor publication adapter was factored so explicit rear hits
and ordinary armor hits use the same injury and evacuation code. Mech impact
publication likewise shares its existing rollback and casualty boundary while
allowing scenario self attribution and full feedback. Ordinary impact callers
retain their existing policy.

Acceptance spans seven chassis, ordinary/rear/lethal critical damage, real armor
loss and destruction, native/Lua state and dice equality, rejected inputs, wizard
permissions, callback rollback and restart. Crew, vehicle critical and live firing
regressions cover the touched shared paths. The separate @DAMAGE command remains
open: its reference packet calculation uses clustersize packets of integer-divided
damage, rather than the split implied by the help text. That behavior needs its own
implementation and acceptance, alongside the remaining scenario/field audit.

Verification: 60 distinct focused tests passed: 48 damage/crew/critical/vehicle-fire
checks (`target/scenario-damage-final-tests.log`), 5 access checks
(`target/scenario-damage-access.log`) and 7 help checks
(`target/scenario-damage-help.log`). Strengthened material-loss/destruction
assertions passed (`target/scenario-damage-acceptance.log`). All-target Clippy with
warnings denied passed (`target/scenario-damage-clippy.log`), as did formatting,
diff checks and mirrored Lua declarations. The reference tree is unchanged.
Full-suite refresh and the broader integration acceptance remain outstanding.


## Vehicle hit-arc correction

Tracing wizard packet damage exposed a shared direction error: vehicle targets
were classified with Mech front arcs. `combat/mech_hitloc_targeting.c::mech_hit_group`
uses front/rear half-widths of 30/30 degrees for ground vehicles and VTOLs in
mode zero, 45/45 in mode one, and 90/30 in mode two. Mechs retain 90/30 in modes
zero and two. Boundary angles belong to the front/rear arcs.

The common direction resolver now selects widths from the target chassis, so
Mech and vehicle launchers share the correction. Vehicle beacon attachment,
shot admission and front-only dug-in cover use that same resolver. Fixed blast
arcs and Mech physical-hit tables retain their own specified geometry.

The direction regression checks every degree and both sides of boundaries for
all three modes across all 49 supported shooter/target chassis pairs. The dug-in
aim regression also checks actual cover modifiers for both shooter anatomies.
Wizard `@DAMAGE` remains open: its reference argument named `clustersize` is the
packet count, each packet receives integer `damage / clustersize`, and the
remainder is discarded. Its location routing resets the supplied critical flag
and preserves the rear selector across packets. These details must be carried
through shared impact and casualty handling when the command is added.

Acceptance passed 60 distinct tests: the exhaustive direction library test
(`target/hit-direction-tests.log`) and 59 tests covering digging, Mech and
vehicle salvos, vehicle fire and shot admission, and MML combat
(`target/vehicle-hit-arcs-tests.log`). After the final conditional cleanup,
all seven digging tests passed again (`target/vehicle-hit-arcs-dig-final.log`).
All-target Clippy with warnings denied passed
(`target/vehicle-hit-arcs-clippy.log`), as did formatting and diff checks.
The reference tree remains clean. This is focused regression evidence; the
full integration completion audit remains outstanding.


## Wizard random packet damage

`@DAMAGE damage clustersize isrear iscritical`, guarded Lua
`btech.unit.damage(actor, unit, damage, clusters, rear, critical)` and the exported
`battle_damage_action` now share one atomic scenario action. The reference's
cluster argument is a packet count: 11 damage in three clusters produces three
packets of three damage. Remainders are discarded. Signed numeric native flags
use nonzero truth; location routing chooses critical eligibility regardless of
the supplied critical flag. Rear geometry enables rear damage for the remainder
of the sequence. Self direction uses the same configured chassis arcs as combat
and the reference's same-point bearing of 180 degrees.

The action admits supported physical units without power, pilot assignment or
map placement. Random hit tables retain configured critical policy. Material
packets were extracted from blast handling, so wizard damage, mines, artillery
and reactor blasts share armor, internal transfer, vehicle motive effects and
crew injury handling. Packet traversal continues after destruction. Unplaced
Mechs retain material/crew damage without trying to resolve battlefield falls
or flooding. Located wizard damage also reuses the common attributed-impact
entry, replacing duplicate toughness/attribution setup.

Self-hit cover is sampled from the unit's current sightline. The reference
instead consumes an ephemeral partial-cover flag from whichever observer most
recently queried the unit; that unrelated-observer state is not carried into
Rust scenario damage. No firing expenditure, missile cluster roll, ammunition
selection or woods absorption is introduced by the scenario action.

Reports retain every packet plus per-packet and discarded damage. Effects are
published only within the action checkpoint; late output failure or a failing
Lua callback restores damage, dice and pending output. The native command,
access fixture, wizard help and mirrored Lua declarations are included.

Acceptance covers all seven supported chassis, native/Lua equality with opposite
critical flags, real armor damage, lethal multi-packet traversal, save/reload,
malformed and out-of-range input, denied nonwizard callers, callback rollback,
late output failure, unplaced units, and 1,000 combat-safe packets that preserve
material state while consuming dice. The shared direction regression checks
self-hit and direct classification across all three modes and boundary angles.

Verification passed 56 distinct tests: 55 across scenario damage, mines,
artillery firing, reactor explosions, combat safety, crew, access and help
(`target/scenario-packets-regression.log`), plus the exhaustive direction test
(`target/scenario-packets-direction.log`). All-target Clippy with warnings denied
passed (`target/scenario-packets-clippy.log`). Formatting, diff checks, mirrored
Lua types and the clean reference-tree check also passed. Full integration
acceptance remains open.


## Wizard construction weight report

`@WEIGHT` and guarded Lua `btech.unit.weight(actor, unit)` now publish the same
private allocation report. The exported `battle_weight_report` provides typed
rows and `battle_weight_action` publishes their common rendering. Wizard
permission and a live supported physical unit are required; power, placement
and pilot assignment are not. Trailing native arguments are ignored, matching
the command handler.

Allocation uses original construction and installed bin capacity, including
empty and half-ton bins. It is separate from current physical mass, which still
tracks material loss and remaining rounds for movement and combat. Both Mech
and vehicle reports list components, weapons, systems and ammunition families,
then total tonnage and offset from nominal tonnage. The common layout aggregates
equipment by name; it is not a construction-legality certificate.

Mech per-critical system and cooling arithmetic was extracted from existing
mass calculation for reuse by reporting. Vehicle reporting uses the existing
intact-template mass and system weights. Neither interface maintains a second
set of equipment weights or modifies live state. Literal text is escaped and
report publication rolls back all of its lines on an output-capacity failure;
a failing Lua callback also removes pending report output.

Acceptance passed 55 distinct tests: 22 mass, cargo-space and stock-mass tests
in `target/weight-tests.log`, five access tests in `target/weight-access.log`,
and 28 weight-report, Clan chassis, engine, gyro, C3, electronics and help tests
in `target/weight-final-tests.log`. The two report tests passed again after the
shared-fixture lint annotation (`target/weight-acceptance.log`). Final all-target
Clippy with warnings denied passed (`target/weight-clippy.log`), along with
formatting, diff and mirrored Lua-declaration checks. The reference tree remains
clean. Full integration completion is still unproven and the goal remains open.


## Wizard coordinate positioning

`SETXY x y [z]`, guarded Lua `btech.unit.setxy(actor, unit, x, y, z)` and
`set_battle_coordinates_action` now share same-map scenario positioning. Signed
integer coordinates are checked against map bounds; explicit altitude clamps
to the unit's signed-short range. Omitted altitude selects the destination
surface, with hover water handling, and lands a rotorcraft. Power, crew, heading,
commanded motion, map membership slot and the moved unit's outgoing selection
are retained. Incoming locks and both directions of cached observations are
invalidated through the shared contact service.

A jump now retains an explicit relocated sample until its next committed flight
update. Route, thrust sample and elapsed distance remain unchanged. The override
is validated and persisted with the flight cursor; the next tick continues the
existing path. Forced descent changes altitude while retaining speed/countdown.
The ground-altitude validation range now admits the same signed-short scenario
coordinates for running units, instead of restricting them to map terrain levels.

Tow pairs move atomically through the existing synchronization service, whether
the caller names the carrier or its target. Notifications and positioning share
one rollback checkpoint. Native/Lua equivalence, invalid arguments, permissions,
altitude extremes, restart, late output failure, active jumps, VTOL flight and
landing, and all 49 supported tow pairings are exercised by acceptance tests.

Verification passed 122 distinct tests: five access tests in
`target/setxy-build.log` and 117 positioning, jump, terrain-edit, towing,
visibility, VTOL-control and help tests in `target/setxy-regression.log`.
The four positioning tests passed again with strengthened nonzero motion and
falling-VTOL coverage (`target/setxy-final-tests.log`). The stationary fixture
keeps its desired heading equal to its current heading, as immobile units cannot
retain a turning command. All-target Clippy with warnings denied passed
(`target/setxy-clippy.log`), as did formatting, diff, mirrored Lua types and
reference-tree checks. Remaining scenario/field commands and whole-integration
acceptance are not closed by this delivery.


## Battlefield identity prerequisite for map reassignment

Assigned battlefield IDs now have their own durable field, independent of the
membership slot. The trusted Rust `assign_battlefield_id` helper shares ID
selection across Mechs and vehicles, normalizes a supplied two-byte preference
to uppercase letters, and resolves collisions using the unit's saved dice.
Missing or short preferences request random letters. A bounded collision loop
falls back to the first available letter pair; exhaustion fails atomically.

Ordinary placement retains same-map IDs and avoids collisions with assigned
IDs when choosing an initial label. Slot allocation and membership ordering
remain unchanged. Scanner, targeting and radio projections read the common
identity accessors. Saved IDs are validated for shape and uniqueness per map.
The assigned ID is separate from any future configured preferred-ID setting.

This is an internal prerequisite, not delivery of `SETMAPINDX`: its native/Lua
action, map removal, destination handling and publication still need work.
Acceptance covers all seven supported chassis, unchanged controls and slots,
deterministic collision replay, ordinary placement collisions, same-map
retention, invalid saved IDs and database round trips.

Verification passed 35 distinct tests: identity, radio, vehicle placement and
MML tests in `target/battlefield-id-tests.log`, plus construction tests in
`target/battlefield-id-final-tests.log`. The final run also repeated identity
and placement tests after consolidating collision lookup. All-target Clippy
with warnings denied passed (`target/battlefield-id-clippy.log`). Formatting,
diff checks and the read-only reference-tree check passed. Full integration
acceptance remains open.


## Live scenario map reassignment service

`reassign_battle_map` now provides a shared transactional destination service
for all seven supported chassis. It checks live map/unit objects, containment,
current membership and the reference's 250-member destination capacity before
committing. Existing members may be reassigned on a full map. The helper assigns
an independent battlefield ID using the durable shared identity service.

The service retains crew, power, heading, throttle, continuous position and
active flight state when coordinates fit the destination. Out-of-bounds XY
resets to the origin through the same geometry service used by SETXY. Ground
altitude is retained across changes in terrain height; new unplaced construction
uses the destination surface. Target selections/observations, the unit's TAG
selection and C3/C3i membership are cleared. TAG recycling time remains intact.
Reassigning either member of a tow releases the relationship and leaves its
partner on the original battlefield.

This is the destination portion of SETMAPINDX, with no native/Lua surface yet.
Map removal remains incomplete: the reference removes membership immediately
without shutting down, then its movement-map validation can shut down an
unplaced running unit on a later update. Rust currently rejects powered unplaced
snapshots. That lifecycle, saved off-map coordinates, preferred-ID configuration,
airborne routes that do not fit the destination, and host publication/rollback
still require work before the command can be considered delivered.

Verification passed 45 distinct tests: 44 identity, assignment, SETXY, towing
and placement tests in `target/map-assignment-regression.log`, plus the mixed
tow reassignment test in `target/map-assignment-final-tests.log`. All five
assignment tests passed again with TAG countdown and nonzero throttle checks
(`target/map-assignment-motion-tests.log`). Final all-target Clippy with warnings
denied passed (`target/map-assignment-motion-clippy.log`), along with formatting,
diff and read-only reference-tree checks. The integration goal remains open.


## SETMAPINDX removal lifecycle and host interfaces

Native `SETMAPINDX map [preferred]`, Lua
`btech.unit.setmapindex(actor, unit, map, preferred)` and
`set_battle_map_index_action` now share wizard authority, assignment/removal,
private confirmations and world/effects rollback. Decimal -1 removes membership;
other negative indices and unavailable destinations fail without changing dice.
Native parsing retains the reference's first-two-arguments behavior.

A durable detached flag now distinguishes tactical membership from a retained
physical pose. Public position and identity projections show no map after
removal, while saved coordinates, motion, power and crew remain available for
immediate re-entry. Normal placement/removal clears detached state. Membership
removal also releases towing, clears observations and selections, cancels pending
building entry, and removes TAG and command-network links. Cover preparation is
cleared on removal. The surrounding game object is not relocated by removal.

The next unit update resolves active detached state with shared Mech/vehicle
shutdown cleanup. There is no current terrain on which to apply landing/fall
impacts. Flight stops while altitude and coordinates remain available for later
re-entry. Stationary detached vehicles explicitly keep the scheduler awake for
that update. The normal shutdown paths call the same extracted control-cleanup
functions. Re-entry before the update preserves flight; after it, systems are off.

The earlier missing native/Lua interface and removal-state items are superseded
by this delivery. Configured preferred IDs, destination-incompatible jump routes
and broader scenario/field command and full-integration audits remain open.


Verification passed 2,275 distinct tests across the split workspace run and
final regressions. The initial broad run recorded 1,303 passes
(`target/setmapindex-full-tests.log`); the remaining-target run recorded 726
(`target/setmapindex-remaining-tests.log`); the final general tests recorded 242
(`target/setmapindex-general-tests.log`). Final acceptance added four distinct
successes: the corrected stationary scheduler check, startup/idempotent removal,
former-map purge/re-entry, and the corrected command inventory expectation.
All seven map-index tests and seventeen command tests passed in
`target/setmapindex-final-acceptance.log`.

The initial broad build preceded the final scheduler change and stopped at its
new regression. The later general command check exposed an inventory fixture
that omitted several already-registered wizard commands. Both failures were
resolved and rerun. Fifty-seven jump, five assignment and one runtime-diagnostic
tests also passed against the final lifecycle code
(`target/setmapindex-final-tests.log`); the startup fixture subsequently cleared
its running-only target lock before testing stopped/starting state. The final
acceptance includes retained coordinates after the former map is purged.

All-target Clippy with warnings denied passed (`target/setmapindex-clippy.log`),
as did documentation tests (`target/setmapindex-doc-tests.log`), formatting,
diff, mirrored Lua-type and read-only reference-tree checks. This closes the
host/removal implementation slice, not the remaining parity or integration audit.


## Configured preferred battlefield identities

`BattlePreferredId` is a validated two-letter configuration value shared by
Mechs and vehicles. Setters and deserialization normalize ASCII letters to
uppercase; malformed saved values are rejected. Each owned unit snapshot keeps
its preference separately from its currently assigned battlefield label. Empty
or absent setter values clear the preference without changing the live label,
map membership, crew, controls or durable dice.

ID selection now uses an explicit argument of at least two bytes first, then
the configured preference for omitted/empty/one-character arguments, then the
existing random selection. Collisions retain the existing durable random path
and never overwrite the configured preference. Removal, restart and re-entry
retain the configuration.

Trusted Rust `set_battle_preferred_id` / `battle_preferred_id` and the wizard
`set_battle_preferred_id_action` share that model. Guarded Lua
`btech.unit.set_preferred_id(actor, unit, value)` configures it with callback
rollback; `btech.unit.state(unit).preferred_id` inspects it for either anatomy.
It can be configured before placement. Existing unowned SQL configuration rows
remain preserved; this adds configuration to Rust-owned unit snapshots without
claiming import of opaque reference state.

The configured-ID behavior missing from earlier SETMAPINDX entries is now
implemented for constructed Rust units. Incompatible jump routes and the broader
scenario/field and whole-integration audits remain open.

Verification passed 34 distinct tests: the preferred-ID value test
(`target/preferred-id-unit-tests.log`), 21 identity, assignment, removal and
persistence tests (`target/preferred-id-tests.log`), and twelve help/access tests
(`target/preferred-id-surface-tests.log`). All-target Clippy with warnings denied
passed (`target/preferred-id-clippy.log`), along with formatting, diff, mirrored
Lua-type and read-only reference-tree checks. The prior full workspace audit is
not repeated by this focused configuration delivery, and the goal remains open.


## Reassigned jump routes and destination boundaries

Scenario map assignment now rebinds incompatible jump routes to the destination's
boundary policy. The original route, sampled altitude, thrust and elapsed travel
remain intact. A relocated sample retains the committed physical position until
the next update. Compatible routes do not acquire a new override. A saved marker
distinguishes routes admitted on another map from ordinary launches: ordinary
launches keep full map-specific admission checks, while reassigned routes resolve
future edges at movement time. Saved wrapping dimensions still have to match the
current map.

On a non-wrapping edge, the proposed coordinate clamps to the edge tile and enters
the existing landing service with the enclosing action's fall, mine, stacking,
DFA, experience and casualty context. Wrapped movement uses the destination's
current dimensions. This avoids reimplementing landing mechanics and preserves
transactional publication in the native/Lua host path.

The biped/quad acceptance matrix covers both source wrapping states, both
destination wrapping states, smaller/equal-size maps, exact retained progress
and sampled altitude, save/reload and deterministic continuation to landing.
A host test compares native/Lua reassignment, callback rollback and boundary
landing publication. This supersedes the earlier missing incompatible-route
item for ordinary map edges and wrapping. Broader scenario, reference-state
import and whole-integration acceptance remain open.

Verification passed 87 distinct tests: 75 jump, reassignment, SETMAPINDX,
assignment and SETXY tests in `target/jump-reassignment-regression.log`, plus
five wrapping and seven help tests in
`target/jump-reassignment-boundary-tests.log`. All-target Clippy with warnings
denied passed (`target/jump-reassignment-clippy.log`), as did formatting, diff
and read-only reference-tree checks. The full integration goal remains open.

## Shared orbital-drop rules (host integration pending)

`BattleOrbitalDrop` provides one restartable rules model for Mechs and ground
vehicles. It covers one-second descent steps, mass-based cocoon integrity,
whole-packet damage interception, firing-induced breaches, jump-jet compensation,
the protected target modifier, and landing damage/experience arithmetic. The
landing calculation retains the reference's doubled margin and distinct terrain,
crew, parachute and chassis modifiers. Combat-safe landings require no dice.
Malformed saved protection, invalid rolls and altitude overflow are rejected.

Remaining integration must connect damage interception, firing-induced breaches,
the targeting modifier and `ood_land` callbacks, and complete mixed
combat, shutdown and casualty acceptance. Construction readiness and the full
integration goal remain open.

Verification passed 187 library tests and five orbital-drop integration tests
(`target/orbital-core-tests.log`). All-target Clippy with warnings denied passed
(`target/orbital-core-clippy.log`), as did formatting and diff checks. The reference
tree remains unchanged. Live host acceptance is pending the connections above.

### Orbital-drop state and scenario lifecycle

Both unit records now own the shared optional drop cursor. Validation requires a
physical pose and exclusive altitude ownership, rejects a retired/breached cursor,
and excludes VTOL cocoons. Range, LOS and altitude inspection use the drop height.
Ground movement and ordinary jumping leave the drop alone; flight, towing, hiding
and transfer admission account for active drops. Terrain edits preserve their
height without inventing a second altitude owner.

SETXY updates drop altitude while preserving its protection. SETMAPINDX retains
the cursor across reassignment, including an origin reset on smaller maps.
Tactical removal retains it until the deferred shutdown update, which retires the
drop and keeps its last altitude. Administrative placement, map purge and resolved
falls clear the cursor. Lua `btech.unit.state(unit).orbital_drop` returns detached
inspection data with matching type declarations for both anatomies.

Host tests cover all six ground chassis, persistence, moving-unit exclusion,
native scenario services and Lua relocation/inspection/rollback, conflicting
saved altitude owners and VTOL rejection. The scheduler's work predicate sees
drops. The following deliveries add descent, touchdown and launch actions;
combat connections remain pending. The full integration goal remains open.

Verification passed 411 distinct tests: 208 library, drop, jump-reassignment and
scenario tests (`target/orbital-state-tests.log`), with the three expanded drop
state cases rerun in `target/orbital-state-acceptance.log`, and 203 surrounding
surface, movement, transfer, terrain, hiding, towing, explosion and crash tests
(`target/orbital-state-regression.log`). All-target Clippy with warnings denied
passed (`target/orbital-state-clippy.log`), as did formatting, diff and mirrored
Lua-type checks. The reference tree remains unchanged.

### Orbital descent and touchdown in the airborne update

The existing airborne phase now advances drops for both Mechs and ground vehicles
once per committed second, including stopped units. One adapter selects terrain
support, reads crew facts, rolls durable dice and uses the shared drop model.
Failed landings call existing anatomy-specific fall services with their normal
packet damage, personal injury, terrain, mine and casualty reports. Safe landings
skip dice. Successful landings reuse ice and Mech stacking resolution; submerged
ground vehicles use the existing flooding mutation with hover and waterproof
support rules.

Character landing XP reuses the active-crew and skill award policy with the drop's
explicit reason. Direct pilot roll messages retain their place between touchdown
and crash notices, including when the drop report follows other airborne events.
All effects remain in the existing movement checkpoint. Rejected output restores
altitude, protection, armor, crew, dice and staged messages.

Acceptance covers all six ground chassis, stopped safe descent, save/reload at an
intermediate altitude, single touchdown, deterministic failed landings, submerged
tracked/hover/waterproof outcomes, character XP and ordered pilot feedback, and
late publication rollback. Combat interception/breach behavior,
targeting modifiers and `ood_land` callbacks remain unconnected. Integrated ice,
crowding and casualty edge cases remain part of the full acceptance audit.

Verification passed 419 distinct tests across the library, orbital drop rules and
state, jumps, piloting, scenario relocation, surface effects and vehicle movement.
Results are in `target/orbital-movement-tests.log`,
`target/orbital-movement-regression.log` and
`target/orbital-movement-final-tests.log`; the five expanded drop movement cases
passed again in `target/orbital-movement-acceptance.log`. All-target Clippy with
warnings denied passed (`target/orbital-movement-clippy.log`), as did formatting,
diff and mirrored Lua-type checks. The reference tree remains unchanged.

### Native and Lua orbital insertion

Wizard `@OOD x y [z]`, Lua `btech.unit.ood(actor, unit, x, y, z)` and trusted host
entry `initiate_battle_orbital_drop_action` use one authority/effect checkpoint.
The current map must be live and the coordinates valid; towing must be detached.
Prone Mechs, active digging and an existing cocoon reject before relocation.
Altitude defaults to 300 and follows SETXY's signed-short coordinate bounds.
Insertion uses the shared scenario geometry and observation invalidation, replaces
competing jump/fall state and preserves crew, power, damage, ammunition and dice.
Ground cocoons use current material mass; VTOLs have no cocoon and request half
their maximum horizontal speed.

Airborne VTOL geometry is independent of powered movement. A stopped aircraft
retains its inserted altitude and requested controls, with zero actual horizontal
and vertical speed. Startup can resume ordinary flight after restart. Launch
timers and powered vertical travel still require power; ordinary shutdown retains
its existing loss-of-lift path. The same inert-control validation permits a later
scenario coordinate edit to land the stopped aircraft without discarding its
requested throttle.

Acceptance covers all seven supported chassis through native/Lua commands,
default/explicit/bounded altitude, material-based integrity, save/reload,
unpowered VTOL startup, replacement of an existing jump, rejection and callback
rollback, and output-limit rollback preserving earlier output. The command
catalogue, wizard help and mirrored Lua types include the new action. Combat
interception, breach continuations, targeting modifiers and the landing callback
remain unfinished; the full integration goal remains open.

Verification passed 302 distinct tests across the library, insertion, descent,
scenario commands, VTOL controls/flight/crashes/fuel, ground driving, the command
catalogue and help. Logs are `target/orbital-launch-tests.log`,
`target/orbital-launch-acceptance.log`, `target/orbital-launch-regression.log` and
`target/orbital-launch-final-tests.log`. All-target Clippy with warnings
denied passed (`target/orbital-launch-clippy.log`), along with formatting, diff and
mirrored Lua-type checks. The reference tree remains unchanged.


### Shared orbital combat integration

Cocoon interception now runs at the common Mech and vehicle material entries,
after damage-entry diagnostics, immunity and dead-section routing, before armor,
internal structure and criticals. A protected roll above eight absorbs the entire
packet, including overkill. Internal vehicle explosions use the same handler;
armor overflow does not roll interception a second time. Intercepted direct hits
and dump ignition do not accumulate stagger damage.

One shared adapter owns both firing-induced and damage-induced breaches. Surviving
jump jets retain descent with compensation; other airborne ground units hand their
height to the existing three-second forced-fall clock. The direct, hex and artillery
launch handlers carry the adapter's notices through their existing transactional
reports. Mechanical launch failures precede firing-induced opening, while failed
Streak locks still open protection. The shared aim subtotal exposes a -2 intact
cocoon modifier for both attacker anatomies, returning to zero after a breach.

Acceptance exercises the six ground chassis, full-packet absorption, overkill,
compensation/free fall, state and output rollback, save/reload, target modifiers,
failed Streak locks, internal explosions and combat-safe dice ordering. The landing
callback and broader operator/construction audit remain open; this does not mark
the full integration complete.


Verification covers all 2,307 library/integration tests across
`target/orbital-combat-final-tests.log`,
`target/orbital-combat-remaining-tests.log` and
`target/orbital-combat-schedules.log`. The complete integration target inventory
was checked against those passing results. The full run caught a missing @OOD
access-catalogue entry from the insertion slice and an occupied-coordinate mistake
in the new hex-shot fixture; both were corrected. One scheduling test's raw
SQLite-byte comparison failed during the broad run and passed unchanged when its
13-test suite was rerun separately. Documentation tests and all-target Clippy with
warnings denied passed (`target/orbital-combat-doc-tests.log` and
`target/orbital-combat-clippy.log`). The reference tree remains unchanged.


### Orbital landing callbacks

The airborne host action now delivers `on_ood_land` after the touchdown notice
and before piloting dice, XP or impact damage. Both ground anatomies use one
continuation: it samples live placement and flags after the event, honors drop
cancellation or removal, and uses the existing landing, fall and casualty services.
The material-only API retains the same physical rules without invoking Lua.
Drop-state lookup is shared by movement, aiming and combat.

The existing movement checkpoint covers all arrivals, callback state, damage,
dice and output. Acceptance verifies all six ground chassis, pre-roll combat-safe
changes, exact event context, once-only delivery across restart, errors preserving
prior output, rollback of earlier arrivals when a later callback fails, and map
detachment without stale-geometry landing. This completes the previously pending
landing callback connection; the broader integration goal and remaining operator,
construction and mixed-scenario acceptance are still open.


Verification passed 339 distinct tests in `target/orbital-callback-regression.log`,
covering the library, orbital state/rules/launch/combat/movement, jumps, ground
vehicle movement, forced descent, stacking and scenario placement. All-target
Clippy with warnings denied passed (`target/orbital-callback-clippy.log`), along
with formatting, diff checks and matching production/fixture Lua declarations.
The reference tree remains unchanged.


### Shared named-unit-field foundation and special-object dispatch

A single unit field reader now supplies native @VIEWMECH and guarded Lua
`btech.unit.fields`, with catalogue ordering, case-insensitive prefix filtering,
shared column formatting and literal publication. Initial live projections cover
identity, placement, motion, crew, team, experience multiplier, mass, towing and
orbital protection. Raw coordinate fields convert normalized Rust geometry to the
field interface's scales; detached inspection retains its physical coordinates
while reporting mapindex -1. Surviving jump thrust is shared with cocoon combat.

Native @SETMECH and Lua `btech.unit.set_field` currently implement team and xpmod
through the existing signature/network and experience services. All edits validate
the world and restore state/output on error. Generic @SETSPECIAL/@VIEWSPECIAL
dispatch to these same unit actions or the existing map/gunner actions; no separate
field mutation logic is introduced for the generic commands.

This is an initial field-service implementation, not completion of the unit-field
contract. Unmapped read fields remain unavailable (n/a); all other unit setters,
including construction, status/critical flags, damage encoding and identity edits,
remain to be implemented. The full integration goal stays open. Acceptance covers
all seven supported chassis, native/Lua parity, retained off-map geometry, numeric
validation, callback rollback, persistence, filtering, literal output and generic
map/station/unit dispatch.


Verification passed 234 distinct tests across `target/unit-fields-regression.log`,
`target/unit-fields-final-acceptance.log` and `target/unit-fields-help.log`, including
the library, command/access catalogues, all field service families and cocoon
combat after sharing its jump-thrust reader. All-target Clippy with warnings denied
passed (`target/unit-fields-clippy.log`). Production and fixture Lua declarations
match, and the reference tree remains unchanged.
