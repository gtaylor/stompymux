# Unit-field write inventory

This is a source inventory, not proof of complete semantics. Implemented means a
Rust setter branch exists. Open entries remain follow-up acceptance work under
the [first implementation boundary](btech-first-implementation.md).
Function-valued fields require auditing the callback's actual write behavior;
the type name alone does not establish that contract.

Reference inputs: `btmux-khi/src/btech/scripting/value_catalog.c` and the special
`displayname` handler in `registry_values.c`. Rust source: `src/btech/unit_fields.rs`.
Only the admitted field catalog is included; the original repair, carrier and
unit-type exclusions are unchanged. Both reference files remain read-only.

The 67 entries include 49 setter entries, one partially admitted
setter (`mechdamage`), 14 Rust read-only fields and three other open
entries. Four write contracts therefore remain open for follow-up. Direct writes
to `status`, `critstatus` and `mechtype` explicitly report that they are unsupported;
their existing inspection and normal gameplay services remain available.

| Field | Reference type | Rust write contract |
| --- | --- | --- |
| `displayname` | `SPECIAL_HANDLER` | Implemented |
| `mapindex` | `TYPE_DBREF_RO` | Read-only |
| `id` | `TYPE_STRFUNC_BUF` | Read-only |
| `mechname` | `TYPE_STRING` | Implemented |
| `maxspeed` | `TYPE_FLOAT` | Implemented |
| `unit_era` | `TYPE_STRING` | Implemented |
| `unit_tro` | `TYPE_STRING` | Implemented |
| `templatesp` | `TYPE_FLOAT` | Implemented |
| `pilotnum` | `TYPE_DBREF` | Implemented |
| `xpmod` | `TYPE_FLOAT` | Implemented |
| `pilotdam` | `TYPE_CHAR` | Implemented |
| `speed` | `TYPE_FLOAT` | Implemented |
| `basewalkspeed` | `TYPE_INT` | Implemented |
| `baserunspeed` | `TYPE_INT` | Implemented |
| `heading` | `TYPE_SHORT` | Implemented |
| `status` | `TYPE_BV` | Open |
| `status2` | `TYPE_BV` | Implemented |
| `critstatus` | `TYPE_BV` | Open |
| `critstatus2` | `TYPE_BV` | Implemented |
| `tankcritstatus` | `TYPE_BV` | Implemented |
| `target` | `TYPE_DBREF` | Implemented |
| `team` | `TYPE_INT` | Implemented |
| `tons` | `TYPE_INT` | Implemented |
| `towing` | `TYPE_INT_RO` | Read-only |
| `heat` | `TYPE_FLOAT` | Implemented |
| `disabled_hs` | `TYPE_INT_RO` | Read-only |
| `overheat` | `TYPE_FLOAT` | Implemented |
| `dissheat` | `TYPE_FLOAT` | Implemented |
| `hsengoverride` | `TYPE_INT` | Implemented |
| `heatsinks` | `TYPE_CHAR_RO` | Read-only |
| `last_startup` | `TYPE_INT` | Implemented |
| `C3iNetworkSize` | `TYPE_INT_RO` | Read-only |
| `realweight` | `TYPE_INT` | Implemented |
| `StaggerDamage` | `TYPE_INT_RO` | Read-only |
| `MechPrefs` | `TYPE_BV` | Implemented |
| `mechtype` | `TYPE_STRFUNC_BD` | Open |
| `mechmovetype` | `TYPE_STRFUNC_BD` | Implemented: validated locomotion, retained material and live state |
| `mechdamage` | `TYPE_STRFUNC_BD_BUF` | Partial: all admitted chassis; reconstruction edge cases remain open |
| `centdist` | `TYPE_STRFUNC_BUF` | Read-only |
| `centbearing` | `TYPE_STRFUNC_BUF` | Read-only |
| `sensors` | `TYPE_STRFUNC_BUF` | Read-only |
| `mechref` | `TYPE_STRFUNC_BD_BUF` | Implemented |
| `fuel` | `TYPE_INT` | Implemented |
| `fuel_orig` | `TYPE_INT` | Implemented |
| `cocoon` | `TYPE_INT_RO` | Read-only |
| `numseen` | `TYPE_SHORT` | Read-only (deliberate difference) |
| `fx` | `TYPE_FLOAT` | Implemented |
| `fy` | `TYPE_FLOAT` | Implemented |
| `fz` | `TYPE_FLOAT` | Implemented |
| `x` | `TYPE_SHORT` | Implemented |
| `y` | `TYPE_SHORT` | Implemented |
| `z` | `TYPE_SHORT` | Implemented |
| `targcomp` | `TYPE_CHAR` | Implemented |
| `lrsrange` | `TYPE_CHAR` | Implemented |
| `radiorange` | `TYPE_SHORT` | Implemented |
| `scanrange` | `TYPE_CHAR` | Implemented |
| `tacrange` | `TYPE_CHAR` | Implemented |
| `radiotype` | `TYPE_CHAR` | Implemented |
| `bv` | `TYPE_STRFUNC_BUF` | Read-only |
| `cargospace` | `TYPE_INT` | Implemented |
| `turret0` | `TYPE_DBREF` | Implemented |
| `turret1` | `TYPE_DBREF` | Implemented |
| `turret2` | `TYPE_DBREF` | Implemented |
| `unusablearcs` | `TYPE_INT_RO` | Read-only |
| `maxjumpspeed` | `TYPE_FLOAT` | Implemented |
| `jumpheading` | `TYPE_SHORT` | Implemented: Mech course and active remaining route |
| `jumplength` | `TYPE_SHORT` | Implemented: Mech course, active route and scheduled landing |

## Verified correction

`disabled_hs` was incorrectly accepted by the thermal setter. Its reference
catalog type is `TYPE_INT_RO`, and the underlying script-value writer rejects
it. Rust now rejects writes before mutation for this field and the other seven
in-scope read-only catalog fields. Heat-cutoff regulation remains the owner of
disabled cooling. Heat, dissipation and excess heat retain their separate writes.

Acceptance passed 33 tests across unit fields, heat cutoff and help rendering in
`target/readonly-fields-tests.log`. The all-chassis matrix exercises all eight
read-only names, case-insensitive lookup, otherwise-valid values, native/Lua
errors and unchanged state/projections.

## Remaining work

Audit each open entry against its actual reference write function. Group related
state transitions so Mechs and vehicles share parsing, validation and atomic
publication. Status and compact damage writes need explicit transitions; do not
store masks that can disagree with material state. Existing branches still need
field-specific semantic acceptance and are not proof of full writable parity.

## Buffered callback audit

The reference `descriptor_write_text` dispatcher in `registry_values.c` only
invokes callbacks for bidirectional function types. `TYPE_STRFUNC_BUF` has no
write arm and returns failure through the default branch. Consequently `id`,
`centdist`, `centbearing`, `sensors` and `bv` are read-only, even though their type
names lack the `_RO` suffix. Rust now gives them the explicit read-only error,
and the shared regression matrix checks their native/Lua rejection alongside
the eight scalar read-only fields.

The expanded matrix and the battle-value and mixed-sensor suites passed 32 tests
(`target/callback-field-tests.log`). These checks cover rejection without state
changes and preserve the callback read projections.

`mechtype` and `mechmovetype` use bidirectional handlers that alter the stored
class and movement identities. `mechdamage` and `mechref` likewise use buffered
bidirectional handlers. Those writable contracts must not be inferred from the
read-only callback behavior; the open entries remain open.

## Cargo capacity writes

The reference script-value writer assigns cargo space directly. Rust accepts
nonnegative signed 32-bit values through the shared unit-field transaction and
updates the owned construction attribute. Existing cargo mass and load services
recompute installation mass and reconcile movement limits for every admitted
chassis. Loose stock is retained, including when capacity is reduced to zero.
Negative values and construction-mass overflow are rejected atomically; Rust
does not permit invalid negative installations. No carrier loading is introduced.

Cargo-field acceptance passed 55 tests across unit fields, cargo installations,
loose cargo and help rendering (`target/cargo-field-tests.log`). The all-chassis
edit matrix checks native/Lua agreement, rollback, persisted construction, stock
retention and overloaded movement stopping.

## Original fuel capacity writes

`fuel_orig` assigns the reference baseline capacity without changing current
fuel. Rust implements that operation for admitted fuel consumers (VTOLs),
updating the owned tank definition and saved baseline together. Existing fuel,
including the exhaustion sentinel, survives the edit. Surplus fuel retains its
load through the existing fuel and shared movement calculations. Values must
be nonnegative signed 32-bit integers; other admitted chassis reject this edit
because they have no fuel inventory. Native and Lua use the same transaction.

The focused field, VTOL fuel and cargo suites passed 49 tests
(`target/original-fuel-field-tests.log`), including native/Lua agreement, callback
rollback, restart, invalid values, non-VTOL rejection and exhaustion retention.

## Actual movement writes

The reference assigns actual speed and facing independently of requested speed
and heading. Rust uses one motion edit service for both chassis families and
retains desired controls, continuous position and timers. Speed parses as finite
32-bit floating point, and headings must be integer compass bearings 0–359.
These edits require placement and pass the complete world validator before
publication. Unlike the reference's unchecked assignment, invalid speed ranges
and immobile/inactive motion states are rejected. In particular, changing an
immobile vehicle's actual heading away from its requested heading would leave
an invalid turning order and is rejected without changing either value.

Actual-motion field acceptance passed 384 tests across the movement, vehicle
motion and unit-field suites (`target/motion-field-tests.log`). The new matrix
checks native/Lua equivalence, retained requested controls and position,
restart, callback rollback, numeric bounds and immobile-state rejection.

## Integer coordinate writes

`x`, `y` and `z` use the existing scenario-positioning action. Each edit keeps
the other integer coordinates, preserves controls, and updates hex-centered
continuous position and the active altitude owner together. Tow pairs and
observation invalidation use that service's existing behavior. Values must fit
a signed short; horizontal coordinates must be inside the current map.

This is a deliberate consistency difference from the reference's independent
integer assignments: edits recenter sub-hex XY and use integer elevation, rather
than leaving continuous coordinates inconsistent with the reported hex. Floating
`fx`, `fy` and `fz` use the precise relocation route described below.

Integer-coordinate acceptance passed 29 tests across unit fields and scenario
commands (`target/coordinate-field-tests.log`). The field matrix compares native
and Lua state to the existing scenario action on all seven chassis, verifies
restart and callback rollback, and rejects out-of-map or invalid numeric edits.

## Continuous coordinate writes

`fx`, `fy` and `fz` parse finite 32-bit floats in the reference coordinate scales
(322.5 per horizontal unit and 64.5 per elevation level). The shared relocation
core preserves fractional position and altitude, updates the containing hex,
and keeps the other two axes and requested controls. Tow synchronization and
observation invalidation use the existing services.

Both integer and continuous routes use the same chassis and altitude adapters.
Jump and free-fall relocation now accept fractional altitude; integer commands
retain their existing rounding. Orbital drop state stores integral altitude and
rejects fractional values. Altitude must fit the scenario signed-short range,
and horizontal points must lie on the current map. Unlike independent reference
field assignment, Rust keeps hex membership and precise geometry consistent.

Precise-coordinate acceptance passed 406 distinct tests across unit fields,
scenario commands, VTOL flight and movement. The final movement/field/scenario
run passed 386 tests after the shared synchronization refactor
(`target/precise-field-final-tests.log`); the earlier VTOL run is recorded in
`target/precise-field-tests.log`. The new field matrix verifies fractional
coordinates, retained axes and controls, containing-hex agreement, restart,
callback rollback and invalid-value rejection on all seven chassis.

## Template movement baseline

`templatesp` is independent from live maximum speed. The reference attacker
movement function uses this baseline for the walking/running firing threshold.
Rust retains an edited baseline in owned construction metadata and routes both
Mech and vehicle attacker modifiers through it. Unedited units use authored
speed; hot-myomer, airborne and posture handling retain their existing order.
Edits accept finite nonnegative 32-bit floating-point values, preserve propulsion
and movement state, and survive restart. `maxspeed` now uses the separate
propulsion component described below.

Template-speed acceptance passed 34 tests across unit fields and Mech/vehicle
aiming (`target/template-speed-tests.log`). The all-chassis field matrix checks
actual firing-penalty changes, exact preservation of all other saved unit state,
native/Lua agreement, restart, callback rollback and invalid-value rejection.

## Propulsion dependency audit

`btech-propulsion-audit.md` records the live/template/construction speed
distinction and the observed damage reset events. The shared propulsion
implementation below addresses the actuator-recalculation interaction and
provides `maxspeed` without editing construction speed or manufacturing motive
damage. Broader composed behavior still needs the audit checklist.

## Live propulsion implementation

A shared saved propulsion component now distinguishes an explicit live maximum
from the Mech baseline adopted during damage recalculation. `maxspeed` edits
change the former without touching construction, mass, template speed or motive
damage. Physical destruction/immobility remains authoritative. Edits that leave
invalid current controls or motion are rejected atomically rather than silently
changing those controls.

Mech hip and eligible leg-actuator losses adopt the current template baseline
and retire an explicit live correction. Leg section loss and the audited
environmental section-disable routes also recalculate. Unrelated critical losses
retain the correction. Vehicle motive speed losses reduce an explicit maximum
while maintaining the independent material-damage record. The full propulsion
acceptance checklist remains in `btech-propulsion-audit.md`; these setter branches
do not by themselves establish every composed movement/status contract.

Propulsion acceptance passed 54 tests across unit fields, Mech mobility, vehicle
motive damage and VTOL critical effects (`target/propulsion-final-tests.log`).
The new regression checks independent live/template edits, unchanged material
mass, native/Lua agreement, rollback, restart before and after damage, unrelated
Mech critical retention, actuator baseline adoption and vehicle speed loss.

## Live jump thrust

`maxjumpspeed` edits surviving thrust at standard gravity. The shared propulsion
component retains a corrected baseline including existing jet losses, so an edit
does not repair equipment and subsequent losses subtract thrust once. Mech jump
capacity applies gravity after the shared loss calculation; vehicle orbital
compensation uses the same calculation. Ground speed, template speed and
construction mass remain independent. Values must be finite and nonnegative;
destroyed units cannot receive positive thrust, and edits must fit the flight
model at the lowest supported gravity. This does not add conventional
vehicle jumping or carrier loading.

Jump-speed acceptance passed 95 tests across fields, jumping and orbital-drop
combat (`target/jump-speed-tests.log`). The all-chassis edit matrix checks mass
preservation, native/Lua agreement, gravity, restart, prior and subsequent jet
losses, repeated-loss idempotence and rollback. A focused follow-up adds the
low-gravity flight-capacity overflow rejection (`target/jump-speed-final-tests.log`).

## Tactical pilot injury writes

`pilotdam` accepts 0–6 tactical injuries. Nonfatal edits change the counter and
any tactical recovery source without rolling dice, waking the crew or changing
its recovery countdown. Occupied and empty cockpits share the recovery component.
Fatal edits use the existing tactical injury service to shut down the unit,
release its pilot and clear active casualty state. Invalid values and edits
that would resurrect a destroyed unit are rejected. Assigned in-character
pilots require the existing character casualty service instead.

This retains the reference's counter-edit behavior for nonfatal values while
adding the cleanup required by Rust's six-hit destruction invariant. It does
not edit RPG health, repair equipment or introduce another injury resolution
algorithm. Native and Lua publication share the field transaction.

Tactical pilot-field acceptance passed 39 tests across unit fields, recovery
and empty-crew handling (`target/pilot-field-tests.log`). The new matrix covers
all seven chassis with occupied and empty cockpits, exact preservation of
nonfatal recovery clocks/dice, native/Lua agreement, fatal cleanup, restart,
invalid values and rollback of both state and notices.

## Enemy-contact counter decision

The reference increments/decrements `numseen` on enemy acquisition/loss, resets
it with lifecycle/map membership, and allows direct signed-short edits. Its
only gameplay reader is `autopilot_ai.c`; other readers are persistence and
field inspection. Rust instead derives the count from acquired contacts and
current teams. Writing a separate counter would create state that could disagree
with observations without serving an included gameplay system.

The implementation therefore keeps `numseen` read-only and rejects edits through
the common guard. This is a deliberate clean-room difference, not a claim that
the reference field is read-only. An optional user preference question was sent;
no answer had arrived when the recommended derived-count behavior was adopted.
This is an implementation assumption, not recorded user approval.

The field inventory's 14 read-only entries comprise 13 reference read-only fields
and this one deliberate difference. Active jump-course edits are documented in
`btech-jump-field-audit.md` and the jump-course write section below.

The contact-counter decision passed 34 unit-field and mixed-sensor tests
(`target/contact-counter-tests.log`), including all-chassis native/Lua read-only
rejection, unchanged state and the existing acquisition/team count checks.

## Secondary status edits

`status2` now edits existing controls and committed electronic observations
without a duplicate status word. Supported bits are `a/b` Guardian ECM/ECCM,
`c/d/e` disturbance/protection/countering, `f` searchlight, `g/h` Mech stealth
and null signature, `i/j` Angel ECM/ECCM, `k/l` Angel protection/disturbance,
`o` automatic turret, `w` fortification, `x` weapons hold and `y` gunnery-XP
suppression. Suite modes are exclusive, and equipment/power/chassis validation
rejects impossible combinations before publication.

Fortification, weapons hold and XP reuse their scenario services. Electronic
observations use existing saved state and will be replaced by subsequent field
refreshes; editing them does not create a physical emitter. Pending searchlight
and signature switch timers retain their existing behavior. Unsupported bits
remain rejected and part of the broader status-transition audit; a setter branch
does not prove complete raw-mask parity.

The common bitvector parser now accepts its own `-` zero-mask presentation, so
reports can be written back without special handling in each status setter.

Secondary-status acceptance passed 40 integration tests across critical and
unit fields (`target/status2-edit-tests.log`) plus the shared mask round-trip
unit test (`target/status2-codec-tests.log`). Existing equipment fixtures now
exercise native/Lua clear-and-restore edits for every projected secondary bit,
including Guardian/Angel modes, lights, signatures, turret control and stored
observations. Invalid combinations roll back all earlier mutations.

## Live mass corrections

`realweight` accepts 0 through 2,147,483,647 in 1/1024-ton units. The shared
saved correction drives gameplay mass for every admitted chassis: load and towing,
effective speed, seismic detection, mine activation, drop launch, stacking,
charge and DFA. Physical construction and component reports still derive their
mass from equipment, protection and ammunition. Editing the correction reconciles
movement limits for the unit and any unit towing it without altering loose cargo.

Reference `mech_script_value.c` assigns `rd.row`; `template_save.c` treats that
value as the current calculated-weight cache. Protection writes in
`mech_equipment_state.c` and ammunition expenditure in `mech_combat_misc.c`
invalidate it. Rust keeps the correction only until protection or ammunition
changes, after which the shared query immediately derives physical mass again.
This avoids carrying cache-validity flags or allowing observer order to choose
when a stale weight becomes visible. Negative masses are rejected rather than
allowing signed mass into gameplay arithmetic. Native/Lua edits share validation,
publication rollback and persistence.

Live-mass acceptance passed 678 distinct tests across the two runs recorded in
`target/live-mass-tests.log` and `target/live-mass-combat-tests.log`; see the
acceptance ledger for the covered targets and the all-chassis edit matrix.

## Nominal tonnage edits

`tons` changes the owned definition and its authored attribute together. Existing
equipment, protection, damage, crew, random state and propulsion settings survive.
Derived construction mass and gameplay calculations use the new tonnage; shared
load reconciliation adjusts this unit and any towing partner. An explicit live
mass correction remains independent, matching the reference assignment to
`ud.tons` without writing `rd.row` (`unit/mech_script_value.c`).

Rust accepts positive 16-bit tonnage subject to its existing construction
constraints, including Mech tonnage of 20–100 in five-ton increments. This is a
validation difference from the reference's unchecked signed integer assignment.
An edit that fails construction or whole-world validation rolls back completely;
it does not rebuild armor, add equipment or heal damage. Vehicle snapshot
validation now also rejects zero tonnage, including directly edited definitions
that bypass template parsing. Broader construction coverage remains an open
acceptance requirement, independent of this setter branch.

Tonnage acceptance passed 71 field, material and movement tests in
`target/tonnage-tests.log`, including exact saved-state comparison for all seven
chassis, native/Lua parity, callback rollback, restart, independent live mass,
invalid numeric/chassis values and zero-ton vehicle snapshot rejection.

## Secondary critical conditions

`critstatus2` edits `a` (consumed hardened-gyro protection) and `b` (light-probe
failure) through saved runtime conditions, independently of material critical
slots. The reference writer assigns this word directly; `crit_mechs.c` consumes
hardened protection before applying ordinary gyro impairment. A raw-bit edit
therefore cannot be represented by destroying or repairing a gyro slot.

Changing `a` preserves current gyro impairment. The next fresh gyro critical
consumes unused protection, or increases impairment when protection was already
used. Hit feedback and balance checks use the same sequence. Changing `b`
disables or restores installed light-probe operation without changing critical
slots or component mass. Disabling an active probe uses the existing sensor
fallback and target-lock clearing. A fresh probe critical reasserts a previously
edited failure. Neither operation adds equipment or repairs material.

Native/Lua writes share the existing whole-world transaction. Unsupported bits,
non-hardened gyro activation, missing probes, and attempts to restore probes in
lost or environmentally disabled sections are rejected atomically. These hardware
bounds are an explicit validation difference from unchecked reference bit writes.
Saved gyro corrections are rejected when their recorded losses exceed actual
losses or when the chassis has no hardened gyro.

The hardened-gyro piloting contribution now preserves event order. Its protected
hit leaves the current contribution unchanged; actuator recalculation then assigns
+2. The first impairing gyro hit adds +3 to the current contribution, and another
recalculation replaces that contribution with +3. Raw protection edits retain the
current contribution. The gyro recalculation acceptance below covers this sequence;
other critical-status setters and their effects remain open.

Secondary-condition acceptance covers 663 distinct tests, including the final
244-test library/field/gyro/help run and 419 combat/movement/probe cases. The
all-chassis matrix verifies native/Lua agreement, rollback, restart, unchanged
material, explicit probe restoration, fresh probe failure and hardened protection
sequencing. The live-impact case verifies balance suppression and hit feedback
after restoring protection on an impaired gyro. Evidence is recorded in
`target/critical-condition-final-tests.log` and
`target/critical-condition-combat-tests.log`.

The implemented `tankcritstatus` write contract is detailed in
`btech-vehicle-critical-audit.md`. Its digging and crew-stun bits cannot be mapped
to synthesized 20/60-second actions: the reference raw writer changes conditions
without scheduling or cancelling their existing events.


Vehicle critical writes now preserve independent conditions and events. Raw
`c`/`d` cover/preparation flags can coexist; neither creates or cancels pending
dig completion deadlines. A new native dig retains older pending deadlines, and
an explicit stop cancels them. Deadlines sharing a tick are coalesced because the
first completion clears preparation before any duplicate callback can act.

Raw `e` changes effective crew stun without changing its recovery countdown.
Normal stun damage restarts the minute and overrides an administrative clear;
recovery clears the condition at its existing deadline. Driving, readiness,
sight, pods and shared radio use the effective boolean, while scheduling uses
pending time. Lua reports both `crew_stunned` and `crew_stun_remaining`.

Turret lock and jam may coexist, retaining heading and repair events. Tail-rotor
writes affect the existing VTOL control condition without damaging main-rotor
material. Unknown bits, missing turret hardware, inappropriate digging chassis,
non-VTOL tail flags and nonzero vehicle flags on Mechs are rejected atomically.
These construction/lifecycle validation bounds differ from unchecked reference
bit assignment. No repair system or carrier loading is added.

The final vehicle-condition acceptance run passed 295 tests
(`target/vehicle-condition-acceptance.log`), including field writes, indefinite
and timed state, overlapping dig deadlines, normal controls, rollback and restart.
The acceptance ledger records the covered targets and earlier combat checks.

## Movement identity writes

`mechmovetype` changes the owned movement identity using shared construction,
identity refresh and load reconciliation. Mechs accept Biped/Quad; ground
vehicles accept Track/Wheel/Hover/None; rotorcraft accept VTOL/None. Names are
case-insensitive. The edit preserves critical slots, protection, damage, crew,
propulsion corrections and pending events. It does not manufacture a new chassis.

Unlike the reference's unchecked identity assignment, Rust validates the new
anatomy and the complete world before committing. Overfilled quad limbs,
incompatible rotor anatomy and invalid live conditions reject the edit atomically.
Changing unit class (`mechtype`) remains a separate open contract.

## Jump course writes

Mechs accept `jumpheading` from 0 through 359 and signed 16-bit `jumplength`
in 1/322.5-hex distance units (322.5 units per hex). Grounded edits retain values
without creating a flight; a subsequent normal launch establishes its own course.
Vehicles reject these conventional jump-course edits; their separately supported
jump thrust remains available for orbital compensation.

Airborne heading changes redirect the remaining distance from the exact current
sample. Length changes set the total course distance, subtracting distance already
flown. A nonpositive or already-completed length requests landing at the current
position on the next tick. Changing heading retains a pending landing; extending
the length to a valid remaining route cancels it. Equal-value edits are idempotent.
Remaining geometry shares flight advancement, terrain collision, DFA admission
and landing, and preserves material, dice and crew state.

The deliberate clean-room difference is coherent redirection to a new endpoint,
rather than retaining the reference's independent old landing destination. This
follows the stated recommended working assumption after an optional question
received no reply; it is not explicit user approval. Range and additional climb
remain bounded by launch-time capacity, with current thrust still controlling
subsequent advancement and falls. Map admission and complete world validation
reject invalid edits atomically.
