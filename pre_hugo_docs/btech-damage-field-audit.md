# Compact damage-field write contract

`mechdamage` remains open. This audit concerns the admitted administrative field,
not the excluded repair command system. Its read projection is implemented in
`src/btech/damage_field.rs`; parsing a list and applying ordinary combat hits
would not satisfy the reference write contract.

## Replacement semantics

`btmux-khi/src/btech/scripting/unit_values.c:124` clones the current unit, resets
its original armor/internal protection, restores non-placeholder critical slots,
refills ammunition and clears non-ammunition temporary failures. It then applies
the supplied records to that replacement state. Consequently omitted records
restore the corresponding material; an empty list is meaningful. Duplicate
records are assignments in input order, rather than accumulated damage.

The admitted record families are `A:section/loss`, `A(R):section/loss`,
`I:section/loss`, `C:section/slot`, `R:section/slot(spent)` and
`G:section/slot(failure)`. Separators in the reference are spaces and commas.
The report's section numbers and zero-based critical slots are stable format
identities, not Rust enum discriminants. Structural placeholders are omitted.
A future Rust parser must validate section/slot existence and numeric limits
before publication, with one shared parser for Mechs, ground vehicles and VTOLs.

The replacement is copied back through slot and protection assignments rather
than ordinary hit routing. Newly destroyed criticals are marked directly. Restored
criticals go through `mech_repair_part`, which has additional equipment-state
consequences: weapon/ammunition restoration clears one-shot, rocket-fired and
jettison flags, while restoration of listed Mech systems invokes `do_magic`
during the copy loop. Those calls need acceptance independently of a slot-bit
change, including any observable ordering or random-state effects.

## Conditional system recalculation

When any critical's destroyed state changes, the handler calls `do_magic` for
Mechs only. Armor/internal-only edits and vehicle edits do not take this branch.
`btmux-khi/src/btech/unit/mech_maintenance.c:325` reconstructs system effects from
an intact template and the current installed equipment plus critical/section
losses. The conditional trigger is important: always recalculating would erase
independent runtime corrections even on a protection-only edit.

Effects requiring explicit decisions and tests include propulsion and jump
thrust, engine heat/cooling, gyro/piloting contributions, sensor ranges and hit
modifiers, primary critical conditions, destruction/revival, equipment specials,
section modifiers, attached iNarc pods, stabilizer losses and surviving one-shot,
rocket and jettison flags. The routine also cancels vehicle-burning events and
clears the performing-action flag. Breach clearing depends on the stall counter.
The script writer reaches this routine only for Mechs, even though the routine
itself also contains vehicle branches; those branches must not be attributed to
vehicle `mechdamage` writes.

Rust should keep the owned construction and shared equipment owners as the
source of these transitions. Reloading a disk template or introducing a repair
command implementation is not required to represent this administrative edit.
The exact recalculation behavior is not yet implemented or accepted.

## Acceptance still required

Exercise all seven supported chassis with mixed protection, critical, ammo and
failure records; empty replacement; omitted and repeated records; malformed and
out-of-range input; exact native/Lua agreement; callback/outbox rollback; and
restart. Separately compare a protection-only edit with a changed-critical edit
while live propulsion, mass, gyro and sensor corrections exist. Cover destroyed
sections, ammunition/weapon destruction, restoration, pending events, casualty
state and destruction/revival without manufacturing combat rolls or messages.
These requirements are not closed by the existing read-projection tests.

## Shared format implementation

`damage_records.rs` now owns `DamageRecord`, its canonical formatter and
`parse_damage_field`. Both Mech and vehicle inspection use that formatter. Parsing
retains input order and repeated assignments; empty input is a valid empty list.
Malformed records, trailing junk, unknown keywords, out-of-range section/slot
indices and signed-integer overflow reject the whole description. Commas and
ASCII whitespace separate records. Unlike the reference's ignored malformed
records, a future field setter will receive an explicit parse error.

Parsed signed losses/failure values are format data, not authorization to create
invalid material. Anatomy, equipment identity, resulting protection/ammo bounds
and supported failure transitions still require replacement-state validation.
No damage-field setter or repair operation is introduced by this codec. Three
new unit tests cover ordered round trips, signed boundaries and malformed input;
an all-seven-chassis integration test round-trips actual damage reports without
changing world state. Existing report output is retained.

## Jump normalization follow-up

The audited `do_magic` calls `do_sub_magic` on its template-derived scratch Mech
before replaying critical effects. In `unit/mech_maintenance.c:215–217`, the
ordinary jet cap multiplies by the integer expression `(2 / 3)`, which evaluates
to zero in C. Thus this particular path zeroes ordinary jump capacity; improved
jets use the other branch. A Rust recalculation must not accidentally reproduce
this arithmetic defect as a meaningful equipment rule. The intended ordinary
walk/run ratio and installed-jet limits need explicit tests when implementing
replacement normalization. This observation does not add or claim that behavior
in the current Rust setter, which remains open.

## Material replacement preparation

`damage_replacement.rs` now prepares complete material assignments through one
shared resolver for all supported chassis. `prepare_damage_field` reads the owned
construction and resolved loadout and returns desired protection, non-placeholder
critical losses, ammunition and weapon failure requests without changing the
world. Section and equipment identities use the same numeric mapping as reporting.

Omitted records restore construction values. Repeated protection/ammunition/failure
assignments retain their last value, and final material amounts are checked after
replacement; an overwritten out-of-range amount does not survive into the result.
Destroyed sections cannot retain armor. Destroyed ammunition bins and sections
produce zero rounds. Empty and structural-placeholder criticals are rejected;
CASE remains real equipment. A loss in a double heat-sink or improved-jet group
expands to the existing combat model's complete installation loss. Weapon failure
requests resolve any occupied mount slot to its primary slot, so repeated aliases
address the same weapon. Known numeric failure codes are 0–7; zero or destruction
removes a requested temporary weapon failure from the prepared result.

This is material preparation, not a live-state setter. Preserved breach/exposure
state, conditional system normalization, destruction/revival, pending actions and
application of temporary failures still need their owners' transitions and final
world validation. Non-weapon temporary failures are not modeled by this preparation
API and remain part of the open field contract. Grouped-loss expansion and
weapon-level failure aliases follow Rust's existing equipment ownership, rather
than introducing independently inconsistent slot fragments.

Four new tests cover all seven chassis, full restoration from omitted records,
ordered overwrites, protection/ammunition bounds, zero rounds after destruction,
invalid section/slot/equipment references, multi-slot weapon failure aliases,
heat-sink/jet groups, CASE versus structural filler, and preparing live reports
back into their current material totals. Every preparation leaves world state
unchanged. Live application is deliberately not claimed by these checks.

## Shared temporary weapon conditions

`weapon_failure.rs` owns compact failure codes 1–7, diagnostic conditions and
recovery-clock policy for both Mechs and vehicles. The trusted
`set_battle_weapon_failure` domain operation changes one existing mount's
condition without generating dice, material damage or a recycle timer. Lua unit
inspection exposes the same `weapon_failures` map for both anatomies. Material
loss takes precedence in reports; normal weapon-destruction cleanup removes its
temporary condition.

A condition without a recycle event persists. Existing recycle events pause
while off and clear temporary conditions on expiry; code 5 completes on the
next powered recycle tick. Ordinary ammunition jams (code 6) can also use the
shared crew unjam action, while critical ammunition jams (code 7) cannot. An
expired crew attempt rechecks that the feed is still jammed before consuming a
roll or ammunition. Compact inspection and weapon diagnostics read this same
state. This supplies the weapon-failure transition needed by replacement;
nonweapon failure records and full live material replacement remain open.

Focused acceptance passed 291 tests across 14 targets, recorded in
`target/shared-weapon-failure-final-tests.log`. The new all-seven-chassis tests
cover all failure codes, compact/diagnostic projections, untouched material and
dice, invalid-index rollback, indefinite conditions, paused clocks, expiry,
restart and crew recovery. Existing vehicle tests retain rejection of failures
on destroyed mounts. This is focused acceptance, not the final project-wide
completion gate.

## Live vehicle material replacement

Ground vehicle and VTOL `mechdamage` writes now use `damage_application.rs`
through the existing native/Lua field transaction. The shared preparation stage
supplies replacement protection, critical slots, ammunition and weapon failures.
Omitted entries restore material; malformed or impossible input rejects the
transaction. Structural filler flags are retained because the compact format
cannot assign them. Restoring an explicitly lost launcher clears its spent flag;
weapon loss retires its recycle event. Temporary weapon conditions and manual
power-down selections are replaced by the supplied failure records.

Vehicle propulsion corrections, crew injuries, critical crew/motive/turret
conditions and sensor corrections are preserved. Existing exposure remains;
failures cannot be attached to unavailable mounts. Material mass corrections
are invalidated only when protection or ammunition changes, and shared load
reconciliation covers towing partners. There is no combat roll, ammunition
explosion or simulated hit routing. Independent crew death, flooding and
transport loss are not undone by material restoration.

Rust derives hull destruction from material. Newly destroyed hulls therefore
use existing wreck cleanup; restoring protection leaves power off and does not
reassign a pilot. This keeps the saved world coherent instead of preserving a
reference status flag inconsistent with its hull. Rotor loss similarly retires
lift. Section beacons cannot remain attached to destroyed sections. These
lifecycle consequences are part of the Rust material model, not the excluded
repair commands.

The vehicle milestone did not include Mech reconstruction. The extension below
adds it; nonweapon `G` records and broader reconstruction acceptance remain open.

Vehicle application acceptance passed 270 tests across nine targets in
`target/damage-application-final-tests.log`, including three new integration
cases exercised across all five supported vehicle chassis. The existing shared
replacement, failure, vehicle critical, VTOL critical, unjam and field suites
also passed.

## Mech application and shared replacement owners

`damage_material.rs` resolves typed section, critical and ammunition assignments
for both anatomy families. `damage_weapons.rs` owns failure replacement, recycle
retirement, manual-jam clearing and restored-launcher replenishment for all seven
chassis. Their adapters retain exposure and reconcile material-dependent actions;
Mech breach ammunition remains unavailable. Mass corrections are invalidated
against final material, after exposure has been applied.

Mech critical-slot changes additionally call `damage_recalculation.rs`. It adopts
owned propulsion and installed-jet baselines, rebuilds gyro conditions from
material, removes sensor-range corrections, clears iNarc effects and restores
surviving launcher availability. Ordinary jet capacity uses the functional
walk/run ratio and installed jets; improved installations use complete pairs.
Protection-only edits retain these corrections. Material restoration cannot
retain gyro corrections whose loss baseline no longer exists. Radio settings,
crew state, heat samples and dice are not reset. Engine and sensor effects
continue to derive from authoritative equipment loss.

The field now accepts Mechs, but completion remains unproven for reconstruction
edge cases: changed engine-sink allocation, secondary critical-condition fidelity,
exposure clearing tied to the reference stall counter, and action ordering while
flight, towing and crew recovery are active. Nonweapon failure records remain
unimplemented. These are remaining acceptance work, not exclusions.

Mech application acceptance passed 330 tests across ten targets in
`target/mech-damage-final-tests.log`. Three new Mech integration cases cover
conditional normalization, native/Lua and lifecycle agreement, and retired
hardened-gyro loss baselines. The five vehicle chassis remain covered by the
same application target after material and weapon replacement were shared.

## Nonweapon diagnostic failure records

`component_failure.rs` now stores named nonweapon failure codes by typed slot on
Mechs and vehicles. Damage replacement accepts installed system/CASE slots,
retains the last assignment, clears omitted/zero conditions and lets destruction
take precedence. Vacant slots, structural filler, ammunition and unknown failure
codes are rejected. Ammunition has its separate `R` assignment contract.

The reference stores these codes in `unit/mech_equipment_state.c`, independently
of physical destroyed, broken and disabled flags. Its nonfunctional system check
uses those material flags; system failure codes appear in damage and critical
inspection. Rust therefore retains them for reporting and persistence without
inventing lost engine, actuator, cooling or electronic capability. No recovery
event is manufactured. Physical destruction still wins in reports.

The shared code enum is now `EquipmentFailure`; `ComponentFailure`
provides typed component locations. Weapon failures keep their operational map
and existing firing/recovery owner. Critical-slot inspection now displays weapon
failures through that same owner, as whole-weapon diagnostics already do. Lua
inspection exposes component failure lists on both unit families. Snapshot
validation rejects duplicate or uninstalled component locations.

This closes the named nonweapon failure-record gap. Mech reconstruction edge
cases listed above remain open; the full damage-field acceptance gate remains
unproven.

## Engine heat-sink allocation reconstruction

Mech reconstruction now adopts a saved cooling baseline from owned external
sink installations plus the selected engine allocation. Single sinks contribute
one point; complete Inner Sphere or Clan double-sink groups contribute two.
A zero override selects nominal engine output divided by 25; positive overrides
are capped by the authored total for the internal contribution before external
installations are added. Large positive values cannot overflow. Negative
allocation metadata remains representable but rejects a reconstruction that
would require negative physical capacity, rolling back the field transaction.

The baseline changes only when critical replacement triggers reconstruction.
An override edit or protection-only replacement leaves current cooling intact.
Subsequent material sink losses reduce the adopted baseline through the existing
heat-sink damage calculation. Cooling suppression is clamped to surviving
capacity; its activation and transition clock remain intact. The baseline is
saved, bounded by owned construction, and used by live heat rates, the heat-sink
field and battle-value calculation. Heat samples and stored heat are not advanced
or reset. Construction and physical equipment mass remain independently owned.

Focused tests cover biped and quad single-sink allocation, Inner Sphere and Clan
double sinks, grouped loss/restoration, delayed override adoption, rollback on
negative reconstruction, large overrides and persistence. This closes changed
engine-sink allocation in the reconstruction checklist. Secondary critical
conditions, exposure/stall behavior and active-action ordering still need their
remaining acceptance work.

## Secondary conditions and repair-stall scope

Gyro reconstruction now captures the pre-edit hardened-protection condition,
rebuilds primary impairment against current material, then retains the captured
condition at the new loss baseline. Removing critical slots cannot manufacture
a second use of consumed protection. Subsequent ordinary criticals follow the
same existing protection/damage progression, including after restart. This is
separate from the remaining audit of complete reconstruction action ordering.

Light-probe failure conditions also survive raw slot replacement on Mechs and
vehicles. When derived material availability would change a saved condition,
the shared condition owner retains the pre-edit value. Actual combat criticals
still reassert failure. Section destruction and exposure retain their existing
physical availability gates; condition retention does not reactivate equipment
inside unavailable sections.

Correction to the earlier stall description: `pd.stall` is a repair-bay object
reference, exposed by `mech_repair_stall_dbref` in
`unit/mech_position_state.c:54`, and consumed by repair job admission. It is not
a movement or recovery counter. The positive-stall breach-clearing branch in
`do_magic` belongs to that excluded repair-facility context. Rust does not add
repair-bay state or automatic breach clearing for this field. Existing breaches
remain, and Mech ammunition in breached sections stays unavailable. This follows
the agreed repair-system exclusion rather than leaving a missing movement rule.

## Active weapon, jump and dumping acceptance

Disabled failure code 5 now suppresses Mech weapon explosions through the same
helper as vehicle criticals and manual Gauss power-down. Other temporary failure
codes do not grant that suppression. The reference's `crit_weapons.c` explicitly
includes `FAIL_DESTROYED` when deciding whether a weapon can explode; the Mech
adapter previously checked only its manual power-down set.

Jump replacement retains the committed flight cursor. Removing all jets changes
available thrust immediately, but the next simulation tick owns lost-thrust and
fall resolution. Restoring jets before that tick continues the same trajectory.
Biped/quad checks cover restart, identical replay, unchanged edit-time dice and
normal eventual landing after restoration.

A matching ordinary feed failure preserves its active unjam countdown. Losing
the mount cancels that attempt without a recovery roll. All-seven-chassis checks
exercise successful timed recovery, restart and cancellation.

Correction: Mech reconstruction no longer cancels ammunition dumping. The
reference's `mech_performing_action_set(false)` clears a separate movement-busy
status flag; it does not cancel the dumping event. Rust now retains the dump's
selection and cadence. A restored bin is processed on that cadence; a destroyed
bin completes through the existing next-tick event. Tests cover both Mech
chassis and persisted replay. The earlier unconditional dump cancellation was
an incorrect interpretation and has been removed.

These checks advance active-action acceptance. Paired towing, crew recovery and
full reconstruction ordering still require the remaining focused audit.
