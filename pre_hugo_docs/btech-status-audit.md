# Main status-word audit

The `status` field now projects existing lifecycle state. This audit records
its mappings, the deliberate omission of observer-dependent cover, and remaining
acceptance work. Projection does not close the setter contract.

## Established mappings

| Letters | Reference meaning | Rust authority / acceptance requirement |
| --- | --- | --- |
| a | VTOL landed | `VtolFlightPhase`; include launch countdown and distinguish a completed landing from falling. |
| b, c, k | Torso right/left, flipped arms | `Facing`; preserve `Torso::Both`, rather than reducing it to its effective angle. |
| d, f | Started, destroyed | Power and chassis destruction services; do not infer destruction from power being off. |
| g, i | Jumping, DFA | The admitted jump flight and its DFA target; orbital descent is a separate lifecycle. |
| h | Fallen | Mech posture is available. Audit VTOL crash and rotor-loss transitions before choosing their mapping. |
| l, m | AMS enabled, explosion safety | Existing AMS selection and self-destruct safety. Do not infer either from equipment presence. |
| n | Unconscious | Both unit crew recovery and an assigned character's recovery may affect control; test pilot reassignment and crew loss. |
| o | Towed | Reverse lookup of the shared towing relation, with no additional unit flag. |
| p–t | Target, building, hex, ignition, clearing | Shared target-selection enum. Resolve unit-at-hex semantics and lock clearing before choosing bits; settling and target visibility are independent. |
| u, z | MASC, supercharger | The admitted booster selections, rather than available hardware or transient speed. |
| v, w, x, y | Blinded, combat safe, autocon while shut down, fired | Existing blindness timer, scenario safety, autocon preference and recent-fire state. |
| A | Hull down | Completed hull-down posture, separately from its pending transition. |
| B–E | Special environment, gravity, temperature, vacuum | Shared map conditions; apply the special-rules gate before subordinate flags. |

The reference environment projection is explicit in
`map/map_conditions.c:map_conditions_apply`: special rules are map flag 2;
temperature means below -30 or above 50; gravity means a value other than 100;
vacuum follows its map flag. `unit/mech_runtime_state.c` clears all four unit
bits when special rules are disabled. Rust stores conditions on the map, so the
projection must follow that authority without introducing per-unit copies.

## Observer-dependent partial cover

Letter `e` is not a stable unit property. The reference's
`sensors/mech_los.c:mech_los_terrain_modifier` copies the observer/target pair's
cached partial-cover result onto the target's status word. A later query from a
different observer can replace it. The write happens before the sensor-to-hit
calculation, independently of whether weapons subsequently fire.

Rust's `sight.rs` and `salvo.rs` consume `unit_terrain_los` for the actual
observer/target pair. Those reports already carry the relevant cover result.
There is no authoritative last-observer cover field on the target. Neither
"the target is in water" nor "any observer sees cover" reproduces the reference
field. A pure unit-field read must not run an arbitrary new observer query.

The rewrite keeps sighting read-only and leaves bit `e` clear. This is a deliberate
inspection difference, not a claim that the unit never benefits from cover.
Firing continues to use cover for the actual attacking pair. No last-observer
value or shadow status mask is introduced. This follows the recommended option
presented during the audit; no user reply had arrived when implementation began.

## Raw status writes

Projection does not establish writable parity. A raw mask can request conflicting
states or set bits that have no active cockpit control. Each writable bit needs
an explicit operation or retained administrative state, with shared validation
and atomic publication. In particular, do not emulate a bit clear by resurrecting
material, silently cancel pending flight, or rebuild crew state as a side effect.
The existing user exclusions still apply to carrier and unsupported-unit flags.

Unit-at-hex selection projects bit `p`, matching the reference coordinate-target
command. Other coordinate modes project one of `q` through `t`; settling does
not clear the selected-mode bit. VTOL launch preparation retains `a`. A landed
VTOL with a destroyed rotor also projects `h`; forced descent before settlement
is not a completed fall. Combat-safe descent preserves the rotor.

Further acceptance must cover main-word jump/DFA, booster, towing and casualty
transitions in complete scenarios. `critstatus` now also has a projection; all
raw status setters remain separate open acceptance work.

## Current verification

The existing sight, targeting-mode and VTOL-flight suites passed all 28 tests
in `target/status-audit-tests.log`. Sight tests explicitly compare BattleTech
state before and after inspection, confirming that a new cover-observation write
would change an established contract. These tests verify the present services;
they predate the main status projection and do not by themselves verify it. Formatting and diff checks passed; the reference tree is unchanged.

The main projection run passed 50 tests across critical/status fields, unit
fields, map environment and VTOL flight (`target/status-tests.log`). Its new
matrix covers all seven chassis fixtures, every target-selection mode, settling,
combined safety/blindness/recent-fire state, unassigned crew consciousness,
environment thresholds, merged torso bits, prone posture, launch preparation,
native/Lua agreement, callback output rollback and restart. This is focused
acceptance, not proof of all main-word lifecycle transitions listed above.
