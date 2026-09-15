# BattleTech integration acceptance

The current milestone boundary, validation and prioritized backlog are in
[the first implementation summary](btech-first-implementation.md). The entries
below preserve the acceptance history; later entries supersede earlier ones.
The former open-ended completion gates are follow-up work under the user's
first-implementation closeout scope, not requirements for full reference parity.
The implementation is Rust-only. The reference tree is read-only. Autopilots,
repairs, naval units, aerospace/dropships, infantry, battle armor and loading a
unit into another unit remain excluded.

## Command names

The September 13 catalogue comparison inspected the reference Mech, map, turret,
debug and special-object catalogues against Rust's native registrations and
built-in aliases. It found one missing in-scope name: `AP`, the reference spelling
of the implemented `armorpiercing` control. `AP` now aliases that existing handler;
there is no second ammunition-mode implementation.

Other missing Mech catalogue names belong to excluded scope:

- Aerospace controls: `CLIMB`, `DIVE`, `THRUST`, `CHECKLZ`, `BOMB`, `@CREATEBAYS`.
- Infantry/battle armor: `THRASH`, `ATTACHCABLES`, `DETACHCABLES`, `PERECM`,
  `PERECCM`, `DISEMBARK`, `EMBARK`, `ATTACKLEG`, `SWARM`, `JETTISON`.
- Carrier loading: `ENTERBAY`, `UDISEMBARK`.
- Repair and refit commands: `CHECKSTATUS`, `DAMAGES`, `FIX`, `FIXARMOR`,
  `FIXINTERNAL`, `REATTACH`, `REPLACESUIT`, `RESEAL`, `TOGGLETYPE`, `REMOVEGUN`,
  `REMOVEPART`, `REMOVESECTION`, `REPLACEGUN`, `REPAIRGUN`, `REPLACEPART`,
  `REPAIRPART`, `REPAIRS`, `UNLOAD`, `@MAGIC`, `@FIXEXTRA`.

Personal ECM is gated by the reference's infantry technology flags. Disembarking
creates a MechWarrior unit. Cable controls are registered for MechWarrior units.
These exclusions concern those commands; supported towing and pilot/character
services remain in scope.

The other catalogues have no additional missing names in this comparison.
Registration establishes availability only. Admission, arguments, behavior,
notifications, failure transactions and persistence need separate acceptance.

## Unit fields

The unit catalogue has 67 entries, now each with a value-projection branch. This
is not proof of complete semantics or writable parity.

The write inventory in `btech-field-writes.md` lists all 67 entries: 49 setter
branches, 14 Rust read-only fields and 4 open write-contract entries.
The buffered callback audit additionally established `id`, `centdist`,
`centbearing`, `sensors` and `bv` as read-only. All 13 reject native/Lua edits
before mutation, including case-insensitive names.
The earlier audit corrected an unintended `disabled_hs` setter; cooling
suppression is again owned only by the heat-cutoff service. That correction
passed 33 tests across
unit fields, heat cutoff and help rendering (`target/readonly-fields-tests.log`).

`critstatus` projects equipment losses and gameplay conditions from existing
state. Shared equipment mapping covers both chassis families; probe failures use
the established availability rules. Mech gyro and hip flags distinguish effective
gyro damage and biped versus quad hip losses. Cache-validity bits `m/n/o` and the
per-update heading marker `B` remain clear because Rust does not store these C
implementation details. Player-character initialization `x` and aerospace spin
`y` belong to excluded unit classes.
Acceptance passed 33 distinct tests across status/critical fields, unit fields,
Mech criticals and vehicle critical resolution. The matrix covers installed and
lost electronic systems on all seven chassis fixtures, missing hardware,
searchlight damage, illumination, scenario flags, gyro stages, biped/quad hip
losses, sensor/life-support loss, heat cutoff and Mech stun, plus native/Lua
agreement, output rollback and restart. Evidence is in
`target/critstatus-tests.log` and `target/critstatus-final-tests.log`.

The main-word lifecycle mapping and observer-dependent partial-cover gap are
recorded in `btech-status-audit.md`. The audit does not count as implementation
or as proof of writable status parity.

`status2` projects selected Guardian/Angel modes, the last committed electronic
field observation, searchlight, stealth/null signature, automatic turret,
fortification, weapons hold and suppressed gunnery experience. Reads do not
recompute interference. Personal ECM and mounted-unit flags belong to excluded
classes; movement modes, attack-channel selection and the separate secondary
supercharger bit have no active enabling path outside raw status writes in the
reference. Those raw-write semantics remain part of the status-transition audit.
Acceptance passed 31 distinct tests covering status/critical fields, unit fields,
electronics on both chassis families, stealth and null signature. The field
matrix checks every projected bit, combined flags, all seven chassis fixtures,
saved observations, native/Lua agreement, rollback and restart. Final concealment
fixtures preserve the quad's ammunition when adding a center-torso device.
Evidence is in `target/status2-tests.log` and `target/status2-final-field-tests.log`.

`tankcritstatus` projects turret lock/jam, completed/in-progress digging, crew
stun and tail-rotor damage from existing vehicle state. `critstatus2` projects a
hardened gyro's first or subsequent hit and unavailable installed light probes.
Both use the shared letter-bit formatter. Raw writes to these fields remain
unsupported pending the status-transition audit; no shadow status words are stored.
Acceptance passed 47 tests across critical fields, unit fields, probes, gyros,
digging, rotor damage and turret controls. The new matrix checks every vehicle
bit, hardened gyro damage before stability loss, probe presence and damage,
combined secondary bits, native/Lua agreement, output rollback and restart.
Evidence is in `target/critical-field-tests.log`.

`StaggerDamage` and `unusablearcs` expose zero and reject writes. The reference
counter behind `StaggerDamage` is separate from rolling damage history: its only
runtime assignments reset it or set a negative buffer in a check that requires
a positive value. No gameplay path increments it or calls the check scheduler.
The reference's unusable-arc mask likewise has no active writer. Saved reference
snapshots can contain arbitrary historical values; this does not justify adding
unused mutable state to newly constructed Rust units. Rust's existing stagger
history and physical arc checks remain authoritative for combat.
Acceptance passed 38 tests across unit fields, Mech/vehicle arcs and stagger
scenarios. The counter matrix includes populated damage history, rejected writes,
native/Lua agreement, rollback and restart on all seven chassis fixtures. Evidence
is in `target/readonly-counter-tests.log` and
`target/readonly-counter-stagger-tests.log`.

`basewalkspeed` and `baserunspeed` now retain signed 32-bit administrative values,
defaulting to zero, with shared native/Lua setters and saved state for both chassis
families. Reference inspection found no movement consumer for these reserved
fields; they do not replace or modify physical speed calculations.
Acceptance passed 379 tests across unit fields, Mech/vehicle motion and savedb;
the base-field matrix covers all seven chassis fixtures, integer boundaries,
native/Lua agreement, authorization, rollback, unchanged speed projections and
restart. Evidence is in `target/base-movement-tests.log`.

`hsengoverride` reads the authored `HSEngOverRide` template value, defaulting to
zero, and supports a shared signed 32-bit setter. The value lives in the owned
template attributes, with validation during construction and snapshot loading.
Changing this field does not rebuild cooling or repair equipment. The reference
setter likewise stores the override without recalculation. Construction's
allocation checks and recalculation still belong to the open construction audit.
Field acceptance passed 32 tests across unit fields, template checks, cooling
cutoff and savedb. Authored overrides are accepted across all seven chassis
fixtures; malformed values fail construction. Native/Lua writes cover unchanged
cooling, authorization, rollback and restart. Evidence is in
`target/engine-sink-field-tests.log`.

`stall` was removed because it identifies a repair stall in the reference; it is
not an aerodynamic or movement state. Repair state remains excluded.

The other fields have projection branches; that count does not prove each field's
complete semantics or setter behavior. Damage-field mutation, writable
construction/speed fields, status/critical mutation, and crew/targeting mutation
still need an explicit field-by-field audit. Read-only fields must stay read-only.
Use existing authoritative state and shared services instead of shadow masks or
separate Mech/vehicle calculations.

`mechdamage` now reads armor and structure deficits, destroyed equipment slots,
spent ammunition and temporary weapon failures through one shared formatter.
It supports native and Lua inspection and uses saved authoritative state. Its
setter remains unsupported; the compact report is not a complete combat snapshot.

Damage-report acceptance covers all seven supported chassis fixtures, native/Lua
agreement, inspection rollback, restart, failure codes and destroyed-bin
precedence. A section-destruction check distinguishes equipment such as CASE from
structural filler. The focused run passed 40 tests, including the MML, unit-field,
critical-report, vehicle-failure and help suites. Evidence is in
`target/damage-field-tests.log` and `target/damage-field-unit-tests.log`.

`pilotnum` now replaces or clears the cockpit assignment through the existing
crew and recovery services. Replacements require a live, conscious player in the
unit; the whole field action restores the previous assignment on failure.
`target` now selects another placed unit on the same map without requiring sensor
acquisition, or clears selection with `-1`. It uses the shared selection storage,
settling delay and artillery-adjustment reset. Target visibility and readiness
are still checked by the normal firing path. These are validated administrative
operations; they do not reproduce unchecked raw C pointer assignments.
Acceptance passed 35 distinct tests across unit fields, cockpit assignment,
unassigned crew and targeting modes. The new all-chassis matrix verifies pilot
replacement/clearing, unacquired target selection, eight-second settling,
invalid and unplaced references, denied authority, rollback and restart.
Evidence is in `target/crew-target-field-tests.log` and
`target/crew-target-final-tests.log`.

## Completion gates still open

- Finish and verify the in-scope unit-field contract.
- Audit construction/readiness and critical effects across supported chassis.
- Verify mixed combat, casualties and restart behavior in end-to-end scenarios.
- Reconcile the detailed coverage documents against final behavior, including
  deliberate differences such as functioning MML combat.
- Run the complete test suite and relevant static checks against the final state.

The detailed implementation history remains in `btech.md`, `btech-coverage.md`,
`btech-delivery.md` and `btech-operator-audit.md`. Their chronological entries can
be superseded by later implementation and should not be used alone as proof of
completion.

## September 13 verification

The audit covered all 304 unit/integration test targets: 2,353 tests passed, with
no ignored tests and no integration target omitted. This combines the initial
full run, focused reruns after fixes, and a continuation through every target the
initial run did not reach. It is not a single uninterrupted run of the final
checkout. Documentation testing completed successfully with zero doctests.

The initial run exposed invalid Lua declaration ordering: unit field and SNIPE
declarations followed the module return. Both the production declaration file and
its fixture now keep the return after all declarations; the existing Lua syntax
check passes. Focused verification also covers the AP alias, removal of the
repair-stall field, general command/access behavior and rendering the help corpus.

Local evidence is retained in `target/integration-audit-summary.json` and the
`integration-audit-tests.log`, `acceptance-fixes-tests.log`,
`integration-audit-remaining-tests.log`, `acceptance-alias-tests.log`, and
`integration-audit-doc-tests.log`
files under `target/`. The summary selects the latest result for each target and
checks it against every integration-test source file.

Passing this test inventory does not close the implementation gaps above or
establish complete reference parity.

Final static checks for this audit passed: `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, `git diff --check`, and production/
fixture declaration equality. Clippy output is in
`target/integration-audit-clippy.log`. The reference tree remained unchanged.

Cargo-capacity writes now update owned construction through the shared load
service, preserving loose stock and reconciling speed limits. Native and Lua
edits use the same rollback boundary. Negative capacities and material overflow
are rejected before publication.

Cargo-field acceptance passed 55 tests across unit fields, cargo installations,
loose cargo and help rendering (`target/cargo-field-tests.log`). The all-chassis
edit matrix checks native/Lua agreement, rollback, persisted construction, stock
retention and overloaded movement stopping.

Original fuel capacity (`fuel_orig`) now supports VTOL edits without refueling
or clearing exhaustion. Owned construction and live baseline remain consistent;
shared load reconciliation accounts for fuel above the edited baseline.

Actual-motion field acceptance passed 384 tests across the movement, vehicle
motion and unit-field suites (`target/motion-field-tests.log`). The new matrix
checks native/Lua equivalence, retained requested controls and position,
restart, callback rollback, numeric bounds and immobile-state rejection.

Integer-coordinate acceptance passed 29 tests across unit fields and scenario
commands (`target/coordinate-field-tests.log`). The field matrix compares native
and Lua state to the existing scenario action on all seven chassis, verifies
restart and callback rollback, and rejects out-of-map or invalid numeric edits.

Precise-coordinate acceptance passed 406 distinct tests across unit fields,
scenario commands, VTOL flight and movement. The final movement/field/scenario
run passed 386 tests after the shared synchronization refactor
(`target/precise-field-final-tests.log`); the earlier VTOL run is recorded in
`target/precise-field-tests.log`. The new field matrix verifies fractional
coordinates, retained axes and controls, containing-hex agreement, restart,
callback rollback and invalid-value rejection on all seven chassis.

Template-speed acceptance passed 34 tests across unit fields and Mech/vehicle
aiming (`target/template-speed-tests.log`). The all-chassis field matrix checks
actual firing-penalty changes, exact preservation of all other saved unit state,
native/Lua agreement, restart, callback rollback and invalid-value rejection.

The propulsion audit identified a damage-recalculation gap after `templatesp`
edits, addressed by the implementation below. See `btech-propulsion-audit.md`
for observed reference reset events and the broader required acceptance cases.

The later propulsion implementation adds shared saved live corrections and
Mech recalculation baselines. Hip/eligible leg actuator losses now adopt edited
template speed, while vehicle motive losses reduce edited live speed. The
earlier actuator-baseline mismatch is addressed; the broader propulsion/status
composition audit remains required.

Propulsion acceptance passed 54 tests across unit fields, Mech mobility, vehicle
motive damage and VTOL critical effects (`target/propulsion-final-tests.log`).
The new regression checks independent live/template edits, unchanged material
mass, native/Lua agreement, rollback, restart before and after damage, unrelated
Mech critical retention, actuator baseline adoption and vehicle speed loss.

Jump-speed acceptance passed 95 tests across fields, jumping and orbital-drop
combat (`target/jump-speed-tests.log`). The all-chassis edit matrix checks mass
preservation, native/Lua agreement, gravity, restart, prior and subsequent jet
losses, repeated-loss idempotence and rollback. A focused follow-up adds the
low-gravity flight-capacity overflow rejection (`target/jump-speed-final-tests.log`).

Tactical pilot-field acceptance passed 39 tests across unit fields, recovery
and empty-crew handling (`target/pilot-field-tests.log`). The new matrix covers
all seven chassis with occupied and empty cockpits, exact preservation of
nonfatal recovery clocks/dice, native/Lua agreement, fatal cleanup, restart,
invalid values and rollback of both state and notices.

`numseen` remains a derived read-only enemy-contact count. The reference permits
an independent counter, whose sole gameplay reader belongs to excluded autopilot
code. This clean-room difference was adopted as the stated recommended assumption
after an optional user question received no answer; it is not user approval.
Active `jumpheading` and `jumplength` mutation was audited separately; the
implemented field contract and its acceptance are recorded below.

The contact-counter decision passed 34 unit-field and mixed-sensor tests
(`target/contact-counter-tests.log`), including all-chassis native/Lua read-only
rejection, unchanged state and the existing acquisition/team count checks.

Saved Mech jump capacity now uses the same representability check as the field
editor and flight advancement. Validation tests standard and minimum gravity
before the unit enters runtime; a corrected baseline may exceed the raw field
limit when it compensates existing jet damage, so validation checks surviving
thrust rather than rejecting the baseline itself. The focused regression accepts
such a valid baseline through restart and rejects corrupted capacity at the
exact overflow boundary. All 91 field/jump tests passed
(`target/jump-capacity-validation-tests.log`).

`status2` now has atomic edits for existing controls and committed electronic
observations; the shared bitvector parser accepts the displayed zero marker.
Unsupported secondary bits and remaining raw-status transition semantics still
require the status audit. No parallel mask is stored.

Secondary-status acceptance passed 40 integration tests across critical and
unit fields (`target/status2-edit-tests.log`) plus the shared mask round-trip
unit test (`target/status2-codec-tests.log`). Existing equipment fixtures now
exercise native/Lua clear-and-restore edits for every projected secondary bit,
including Guardian/Angel modes, lights, signatures, turret control and stored
observations. Invalid combinations roll back all earlier mutations.

## Live mass field

`realweight` now has a shared, saved correction used by movement load, towing,
seismic detection, mine triggers, drop launch, stacking and mass-dependent
physical attacks. Protection and ammunition mutations discard the correction;
component reports continue to calculate physical material. The field inventory
records the explicit difference from the reference's delayed cache refresh.

The initial acceptance run passed 90 tests across unit fields, Mech and vehicle
mass, cargo, seismic detection and stacking (`target/live-mass-tests.log`).
The new all-chassis field matrix verifies native/Lua agreement, callback rollback,
restart, numeric boundaries, overload, unchanged component mass, zero-damage
retention and reset after protection damage or MML ammunition expenditure.

The broader regression run passed another 588 tests: all 192 library tests,
356 movement cases, MML combat, orbital-drop launch, vehicle mines, readiness,
unjamming and help rendering (`target/live-mass-combat-tests.log`). Together these
runs cover 678 distinct passing tests. This is focused acceptance of the shared
mass integration, not a final full-project completion audit.

## Nominal tonnage field

`tons` now updates owned construction and the shared world identity index without
rebuilding material or crew state. Mass-dependent queries and load use the new
value, while explicit live-mass corrections remain independent. Both chassis
families share parsing, identity refresh, movement reconciliation and field
transaction rollback. Vehicle snapshot validation rejects zero-ton construction.

The initial acceptance run passed 71 tests across unit fields, Mech and vehicle
mass, cargo and vehicle movement (`target/tonnage-tests.log`). The new seven-chassis
matrix compares exact saved state after changing tonnage, retaining existing
armor damage and all equipment, verifies native/Lua agreement and restart, checks
callback rollback and invalid bounds, and confirms independent live mass. The
first run exposed an unsynchronized world identity index; the final run includes
the shared refresh used by identity and tonnage edits.

The construction regression run passed another 224 tests across the library,
Mech and vehicle engines, vehicle templates, construction reports and help
(`target/tonnage-construction-tests.log`), for 295 distinct passing tests.
Formatting, diff checks and all-target Clippy with warnings denied also pass
(`target/tonnage-clippy.log`). The reference tree remains unchanged. These results
close the tonnage setter's focused acceptance, not the broader construction audit.

## Secondary critical-condition writes

`critstatus2` now edits hardened-gyro protection and light-probe failure through
saved equipment conditions, keeping material critical slots and mass separate.
Clearing hardened protection preserves existing gyro impairment; subsequent hits
consume protection before adding impairment. Observer/occupant hit messages and
balance handling use the same condition. Probe restoration preserves damaged
slots, and a fresh critical reasserts its edited failure on both chassis families.
Missing, destroyed-section or exposed hardware cannot be restored by this field.

The initial field/gyro run passed 45 tests (`target/critical-condition-tests.log`).
The combat run passed 419 tests across movement, jumping and probes
(`target/critical-condition-combat-tests.log`). Its new live-impact regression
restores protection on an already impaired gyro and verifies the protected-hit
notice, absence of a gyro balance check, retained impairment and exact restart.
The separately identified actuator-normalization modifier is addressed by the
ordered gyro recalculation work recorded below.

The final library, field, gyro and help run passed 244 tests after the shared
fresh-probe-hit correction (`target/critical-condition-final-tests.log`). Together
with combat acceptance, this covers 663 distinct passing tests. Formatting, diff
checks and all-target Clippy with warnings denied pass
(`target/critical-condition-clippy.log`). The reference tree remains unchanged.


## Ordered hardened-gyro piloting

The gyro contribution now preserves the reference's ordered transitions:
`crit_mechs.c` adds three only for the first impairing hit; `crit.c` actuator
normalization replaces the gyro contribution with two for consumed hardened
protection or three for an impaired gyro. A protected hit alone does not add a
modifier. Thus protection, actuator damage, then impairment produces a temporary
+5 gyro contribution; another actuator recalculation replaces it with +3.
Repeated critical requests leave the state unchanged. Saved contributions are
validated and survive restart; ordinary gyros retain their existing calculation.

Speed and gyro normalization now share one Mech method. Auditing
`environment_damage.c:mech_parts_destroy` also established that torso destruction
must recalculate, in addition to leg loss and exposure. Section-loss and exposure
paths share the same anatomy rule: biped arms are exempt; quad arms count as legs.
The source reference tree is unchanged.

Final gyro recalculation acceptance passed 475 tests across gyro construction,
mobility, critical/unit fields, ground movement and jumping
(`target/gyro-recalculation-tests.log`). New biped/quad cases cover damage order,
protected/impairing hits around recalculation, repeated-hit idempotence, torso
versus arm destruction, retained +5 state through world restart, and corrupt
saved-contribution rejection. Formatting, diff checks and all-target Clippy with
warnings denied pass (`target/gyro-recalculation-clippy.log`). The eight remaining
field write contracts and broader integration acceptance remain open.

## Vehicle critical-status timer audit

The `tankcritstatus` setter requires independent digging/stun conditions
and timers; a raw flag assignment does not start a normal timed command. The
source contract, consumer inventory and required transition matrix are recorded
in `btech-vehicle-critical-audit.md`. The implementation is recorded below.

The audit found and corrected a separate scheduler omission: digging countdowns
now keep the shared simulation pending even when a solitary stationary vehicle
has no scanner peers or other active timers. The regression covers tracked and
wheeled vehicles, all twenty ticks, mid-dig restart, and return to idle on completion.

Digging and runtime-diagnostic acceptance passed all nine tests
(`target/dig-scheduler-tests.log`). Formatting, diff checks and all-target Clippy
with warnings denied also pass (`target/dig-scheduler-clippy.log`). The reference
tree remains unchanged. No vehicle critical-status setter is claimed by this audit.


## Vehicle critical-status conditions and events

`tankcritstatus` now writes all six admitted flags through existing vehicle
owners. Digging state exposes separate `dug_in` and `digging` booleans plus a set
of pending completion deadlines. Raw writes preserve those deadlines; native
preparation adds a deadline and explicit cancellation removes it. This supports
indefinite raw preparation, simultaneous cover/preparation and overlapping native
preparations after a raw clear. The scheduler checks deadlines rather than flags.

Effective crew stun is independent of recovery time. Raw writes retain pending
recovery, normal damage restarts its minute, and expiry clears stun. Gameplay
consumers use the effective condition, and the shared radio interface now asks
for a boolean rather than treating a countdown as a condition. Status and Lua
reports distinguish indefinite stun/preparation from scheduled completion.
Turret and tail-rotor flags preserve material, heading and pending repair work;
lock and jam can coexist. Unit-field transactions own validation and rollback.

Final acceptance passed 295 tests across the library, vehicle critical/unit
fields, digging, diagnostics, crew control damage, turret controls, readiness,
unjamming, orbital launch, fire targets and help
(`target/vehicle-condition-acceptance.log`). The all-chassis matrix checks every
flag, valid simultaneous flags, missing/chassis-invalid flags, no material loss,
native/Lua equality, rollback, restart, indefinite conditions, pending timer
clear-and-set, radio/weapon stun admission and normal damage refresh. Separate
dig tests preserve overlapping deadlines through restart and later raw edits.
Earlier combat regression passed 104 cases across vehicle fire/driving, sight,
radio, map messages, orbital launch and fire targets
(`target/vehicle-condition-combat-tests.log`). Formatting, diff checks and
all-target Clippy with warnings denied pass
(`target/vehicle-condition-final-clippy.log`). The reference tree is unchanged.

## Movement identity edits

`mechmovetype` now changes owned locomotion through shared construction and load
services. The seven-chassis matrix covers Biped/Quad, Track/Wheel/Hover/None and
VTOL/None changes, exact saved-state preservation apart from movement identity,
native/Lua agreement, callback rollback, restart and invalid names/classes.
Separate cases reject overfilled quad limbs and incompatible pending hull-down
or digging transitions without losing their equipment, flags or deadlines.
The shared identity projection now records quads with movement code 8 instead
of the biped code 0; construction and subsequent edits use that same projection.

Final acceptance covers 724 distinct passing tests across the library, unit
fields, mobility, movement, jumping, vehicle driving/templates, digging, VTOL
flight and help. `target/movement-field-final-tests.log` records the library and
motion suites; a case-sensitive error-text assertion was corrected in the new
live-condition test, then all field and remaining suites passed in
`target/movement-field-final-validation.log`. All-target Clippy with warnings
denied passes (`target/movement-field-clippy.log`), as do formatting and diff
checks. The reference tree remains unchanged. Six write contracts and the
broader integration acceptance gates remain open.

## Full-project baseline and jump continuation foundation

The full `cargo test --all-targets` run completed successfully: 2,393 passing
tests across 308 targets, no failures and no ignored tests
(`target/btech-project-acceptance.log`). Its binaries were compiled before the
subsequent jump-continuation changes, so this is a verified project baseline,
not final-checkout acceptance of the complete integration. Documentation tests
are a separate check.

The shared jump path can now begin at an exact fractional airborne altitude,
and the flight cursor can replace a remaining segment while retaining its
committed sample, cumulative distance, thrust sample and DFA intent. Status and
altitude rounding account for earlier segments. Lua flight-state declarations
include the saved continuation marker and completed distance. No second movement
integrator is introduced. Details and remaining field requirements are in
`btech-jump-field-audit.md`; field wiring followed this foundation, as recorded below.

Focused acceptance passed 310 tests across the library, jumping, map links and
wrapping, status, unit fields and help (`target/jump-continuation-tests.log`).
The five new unit cases verify exact origin/restart, bounded admission, repeated
redirection, retained intent, progression/arrival and invalid saved state. Three
unused-result warnings in the test setup were corrected afterward; production
code did not change as a result of those warnings.

The newly recorded `btech-damage-field-audit.md` establishes replacement rather
than additive semantics and conditional Mech system recalculation for the open
`mechdamage` write contract. It does not claim that setter is implemented.

The final library rerun passed all 197 tests without warnings
(`target/jump-continuation-final-lib.log`). All-target Clippy with warnings denied
passed (`target/jump-continuation-clippy.log`); the documentation-test command
completed successfully with zero doc tests (`target/jump-continuation-doc-tests.log`).
Formatting and diff checks pass, and the reference tree remains unchanged.
Six field write contracts and the broader completion gates remain open.

## Active jump-course field writes

`jumpheading` and `jumplength` now update retained Mech course values and active
remaining-route geometry through `jump_fields.rs`. Edits preserve the committed
sample and cumulative travel. Length uses signed field units and total distance;
nonpositive or already-completed lengths schedule existing landing handling on
the next tick. Heading retains pending landing, while a valid extension can
cancel it. New takeoffs retain their existing admission and course initialization.
Vehicles reject conventional course edits without creating flight state.

Continuation endpoints may lie in the current hex. Field admission checks the
new path on the current map, resolves endpoint terrain through wrapping, and
clears the old reassignment marker after a valid replacement. The existing flight
integrator and collision/DFA/landing services own every subsequent outcome.
The coherent-redirection difference and its unconfirmed working assumption are
explicit in `btech-jump-field-audit.md` and the field inventory.

Final acceptance passed 316 tests across the library, jumping, unit fields,
map links/wrapping, status and help (`target/jump-field-acceptance.log`). New cases
cover both Mech layouts, grounded/airborne edits, exact unrelated-state retention,
native/Lua agreement, idempotence, malformed and invalid values, callback/outbox
rollback, restart, progression, landing, nonpositive and already-completed lengths,
short same-hex continuations, pending-landing extension, retained DFA intent,
wrapped seam crossing, editing after scenario reassignment, and redirected hill
collision with deterministic replay. Five vehicle chassis reject edits atomically.

All-target Clippy with warnings denied passes (`target/jump-field-clippy.log`).
Formatting and diff checks pass; the reference tree remains unchanged. The field
inventory now records 49 setter branches, 14 read-only fields and four open write
contracts: `status`, `critstatus`, `mechtype` and `mechdamage`. The remaining broader
integration gates are unchanged; this is not full-project completion acceptance.

## Shared compact damage format

Damage inspection now formats typed `BattleDamageRecord` values for both Mechs
and vehicles. The same codec parses complete ordered replacement descriptions,
retaining duplicate assignments and signed numeric values while rejecting malformed
records, unknown keywords, invalid numeric identities and overflow. Format-level
parsing does not apply material changes or waive equipment/state validation.
The damage-field setter and its conditional system recalculation remain open.

Final codec acceptance passed 241 tests across the library and unit fields
(`target/damage-codec-final-tests.log`). New tests cover all record families,
empty/duplicate descriptions, signed boundaries, malformed later records and
round-trips of live protection/ammunition reports for all seven chassis without
state changes. Existing report strings are preserved. All-target Clippy with
warnings denied passes (`target/damage-codec-clippy.log`); formatting and diff
checks pass. The reference tree remains unchanged.

The recalculation audit additionally identified the reference's integer `(2 / 3)`
jet-cap defect in `do_sub_magic`; the required normalization work is recorded in
`btech-damage-field-audit.md`. This finding is not a completed damage setter.
Four field write contracts and the broader completion gates remain open.

## Shared damage replacement material

The new read-only replacement builder resolves compact damage records against
owned construction and loadouts for Mechs, ground vehicles and VTOLs. It computes
complete protection, critical-loss, ammunition and weapon-failure assignments
with ordered overwrite semantics and shared bounds. It uses existing heat-sink
and improved-jet grouping, rejects vacant/filler equipment and wrong record kinds,
and empties structurally destroyed ammunition. No live unit mutation or repair
command is introduced. Runtime failure application, retained exposure, conditional
Mech recalculation and lifecycle reconciliation remain open setter work.

Final acceptance passed 245 tests across the library, replacement preparation and
unit fields (`target/damage-replacement-final-tests.log`). Four new cases exercise
all seven chassis, omitted/duplicate records, live-report material reconstruction,
invalid bounds and equipment, grouped losses, multi-slot weapon aliases and CASE
versus filler, with unchanged world state. All-target Clippy with warnings denied
passes (`target/damage-replacement-clippy.log`); formatting and diff checks pass.
The reference tree remains unchanged. Four field write contracts and broader
integration completion gates remain open.

## Vehicle damage replacement

Ground vehicle and VTOL `mechdamage` setters now publish shared prepared material
through the administrative transaction. Native/Lua parity, complete restoration,
ordered assignments, invalid-input and callback rollback, restart, hull lifecycle
and restored launcher availability have focused coverage across all five vehicle
chassis. Independent propulsion, crew, dice and construction corrections survive
critical replacement. The focused run passed 270 tests across nine targets
(`target/damage-application-final-tests.log`).

This adds a partial setter branch to the inventory; it does not close the full
`mechdamage` contract. Mech conditional system reconstruction and nonweapon
failure records remain open, alongside `status`, `critstatus` and `mechtype`.
The complete integration still requires the final acceptance gates above.

## Mech damage replacement and shared material owners

Mech damage fields now use the same typed material resolution and weapon-state
replacement as ground vehicles and VTOLs. Changed critical slots trigger a
separate Mech reconstruction step; protection-only edits retain gyro, propulsion
and sensor corrections. The focused run passed 330 tests across ten targets
(`target/mech-damage-final-tests.log`). New checks cover native/Lua agreement,
rollback, restart, head destruction/restoration, conditional correction reset,
installed jump capacity and hardened-gyro loss baselines.

The field remains partially accepted. The damage audit records remaining
reconstruction edge cases and nonweapon failure records. This does not close
the full field contract or replace the final project-wide acceptance gate.

## Nonweapon failure records and consistent equipment inspection

Damage replacement now accepts named failure codes for installed nonweapon
components. They persist by typed slot and appear in compact damage and critical
reports; physical system operation still depends on material state. Weapon
critical reports use the same operational failure map as weapon diagnostics.
Both unit families expose component failures to Lua. Duplicate and nonexistent
component locations are rejected during snapshot validation.

The focused run passed 259 distinct tests across seven targets
(`target/component-failure-final-tests.log`). The strengthened weapon-report
cross-check then passed both weapon-failure tests
(`target/component-weapon-report-tests.log`). Component cases exercise all seven
chassis, named codes, last assignment, clearing, material precedence, exact
unrelated-state retention, invalid input and snapshots, and persistence.

The nonweapon-record gap is closed. Reconstruction edge cases and the remaining
field contracts still prevent full integration completion.

## Reconstructed cooling allocation

Critical replacement now adopts a saved Mech cooling baseline using installed
external sinks and the configured engine allocation. Metadata/protection-only
edits preserve current capacity; subsequent grouped sink losses reduce the
adopted baseline. Heat samples and stored heat remain unchanged. Heat rates,
field inspection and battle value consume the shared capacity; construction
and physical mass retain their separate material accounting.

Focused acceptance passed 267 distinct tests across seven targets: 254 in
`target/reconstructed-cooling-tests.log` and 13 battle-value/mass regressions in
`target/reconstructed-cooling-value-tests.log`. Tests cover both Mech chassis,
single sinks, Inner Sphere and Clan double-sink groups, restoration, delayed
adoption, negative-allocation rollback, large positive values and restart.

Changed engine-sink allocation is implemented. Secondary critical preservation,
exposure/stall behavior and active-action ordering remain reconstruction work.
In particular, the reference copies primary critical conditions during its
rebuild while preserving secondary conditions; Rust's current gyro rebuild
still resets its protection condition from material. That distinction requires
a dedicated correction and regression coverage before closing the field.

## Secondary conditions during reconstruction

Material replacement now retains hardened-gyro protection independently of
rebuilt primary damage, and preserves light-probe conditions across raw slot
changes. Existing combat criticals still advance protection or assert probe
failure. New tests cover both Mech chassis, all five vehicle chassis, restart,
restoration and subsequent combat hits.

The repair-stall audit is resolved: the reference's stall value is a repair-bay
object reference, not a movement counter. Its breach-clearing branch belongs to
the excluded repair-facility context. All-seven-chassis tests confirm that
replacement retains exposure, keeps affected weapons unavailable, and preserves
Mech breach ammunition restrictions through restart.

Focused acceptance passed 267 tests across six targets
(`target/secondary-reconstruction-final-tests.log`). This advances reconstruction
acceptance but does not close action-ordering and live-action checks, the other
open field contracts, or the final integration gates.

## Reconstruction during active weapon and movement actions

A shared predicate now applies disabled-weapon explosion suppression to Mechs
and vehicles. Ammunition dumping retains its event cadence during critical
reconstruction. New checks cover Gauss failure codes, airborne jet loss and
restoration, active feed recovery, dump cadence and next-tick completion.

The focused run passed 293 tests across eight targets
(`target/damage-active-final-tests.log`). Towing, crew recovery and complete
reconstruction ordering remain acceptance work; this is not a final integration
completion claim.
