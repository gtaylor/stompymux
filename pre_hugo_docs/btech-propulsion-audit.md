# Propulsion field audit

`maxspeed` now has a shared saved propulsion implementation. The earlier
`templatesp` actuator-baseline mismatch is addressed by explicit recalculation
events. The complete acceptance checklist below still requires evidence beyond
field read/write tests.

## Observed contracts

The reference script-value writer assigns `ud.maxspeed` and
`ud.template_maxspeed` independently (`unit/mech_script_value.c`). Template
loading initializes both from the same authored speed. The attack movement
calculation uses template speed, including the hot-myomer adjustment
(`combat/mech_bth_movement.c`).

Mech hip criticals and leg actuator criticals with an operational hip call
`mech_actuator_criticals_normalize` (`combat/crit_mechs.c`). That routine begins
by replacing live maximum speed with template speed, then reapplies existing
leg and actuator losses (`combat/crit.c`). Section environmental loss also
invokes normalization (`combat/environment_damage.c`). These are concrete
recalculation events, not a reason to apply a template edit to propulsion
immediately. The reference gyro branches and other destruction paths still need
an event-by-event audit before wiring reset hooks.

VTOL rotor damage compares current live maximum against one movement point,
then lowers it or destroys the rotor (`combat/crit_vehicles.c`). Vehicle damage
therefore cannot always be reconstructed by subtracting damage from an edited
construction speed. Speed correction separately applies load and clips requested
and actual speed (`combat/crit.c`). Raw field assignment does not itself call
that correction routine.

## Original mismatch that motivated the change

`Mech::mobility` derives propulsion from `definition.max_speed` and current
material damage. `Vehicle::maximum_speed` derives it from that same
construction value and `motive_speed_loss`. The template-speed edit correctly
leaves these unchanged immediately, but a later Mech actuator loss still starts
from construction speed, rather than the edited template baseline.

Construction speed must remain independent: Mech mass derives engine output
from `definition.max_speed` (`src/btech/mass.rs`). Reusing that value for a live
administrative speed edit would silently change construction mass and engine
accounting. Changing `motive_speed_loss` to emulate a live maximum would also
invent damage and could not represent increases above authored speed.

## Required implementation and evidence

Use a small shared propulsion component with explicit live correction and
recalculation operations. Keep construction speed immutable under these fields,
retain the independent template baseline, and store only the additional state
required to represent an administrative live maximum until its actual reset
event. Chassis adapters own their material consequences; parsing, finite-value
validation, persistence and speed reconciliation should be shared.

Acceptance must demonstrate all of the following:

- A live maximum edit changes available propulsion while preserving construction
  mass, engine output, template speed, material damage and current controls
  except where Rust's documented validity constraints require rejection.
- A template edit changes the firing threshold immediately, leaves live
  propulsion unchanged, and becomes the Mech recalculation baseline at the
  audited damage events.
- New Mech hip/actuator and environmental section losses reapply all relevant
  existing damage exactly once; unrelated weapon or ammunition damage does not
  accidentally reset an administrative propulsion edit.
- Vehicle/VTOL motive effects operate on the correct live baseline, including
  zero-speed and rotor-loss boundaries; structural immobility cannot be bypassed
  accidentally by a numeric field.
- Heat, cargo, towing, boosters, road allowances and configured movement limits
  still compose in their established order, without changing firing thresholds.
- Native and Lua edits agree and roll back atomically. Restart preserves both
  edited states and their subsequent damage/recalculation behavior.

The pending raw status/critical setters must use these same operations when
they change mobility. They must not introduce an independent speed cache or
reinterpret the construction definition to make validation pass.

## Implemented state separation

`Propulsion` stores an optional live correction and the optional baseline adopted
at a Mech recalculation. Existing units use construction and material damage
when neither is present. These values are serialized with each owning unit and
validated on load. Vehicle motive losses lower a live correction once; Mech
normalization retires it and adopts `template_speed`. Constructor and mass
inputs remain unchanged.

The hip/eligible actuator, section-loss and environmental-disable hooks are now
connected. Section loss excludes biped arms, includes quad arms and torsos, and
shares its anatomy rule with environmental exposure. Speed and ordered hardened-
gyro piloting normalization use the same operation. Tests must still be broadened to all interactions listed above; in
particular, the pending raw status/critical setters need the same operations.

Propulsion acceptance passed 54 tests across unit fields, Mech mobility, vehicle
motive damage and VTOL critical effects (`target/propulsion-final-tests.log`).
The new regression checks independent live/template edits, unchanged material
mass, native/Lua agreement, rollback, restart before and after damage, unrelated
Mech critical retention, actuator baseline adoption and vehicle speed loss.

Saved Mech jump capacity now uses the same representability check as the field
editor and flight advancement. Validation tests standard and minimum gravity
before the unit enters runtime; a corrected baseline may exceed the raw field
limit when it compensates existing jet damage, so validation checks surviving
thrust rather than rejecting the baseline itself. The focused regression accepts
such a valid baseline through restart and rejects corrupted capacity at the
exact overflow boundary. All 91 field/jump tests passed
(`target/jump-capacity-validation-tests.log`).
