# Unit status display

`status` uses the reference cockpit layout for the supported units: biped and
quad Mechs, tracked/wheeled/hover/stationary ground vehicles, and VTOLs. The
previous unit-class exclusions remain unchanged.

From inside a unit, enter `status` for the complete cockpit report. Use
`status armor` for the ASCII protection diagrams, `status weapons` for weapons
and ammunition, `status info` for movement and targeting, `status h` for the
heat bar, or `status s` for a short summary. These are read-only commands;
viewing a report does not advance the simulation.

The default order is identification and condition banners, armor diagrams,
movement/heat or flight information, targeting/towing, and the weapon/ammunition
table. An active self-destruct countdown remains visible among the condition
banners. Select `A`, `I`, `W`, `H` or their full names for individual blocks.
`S` selects the short location/heading/speed/condition line and any target.
`R` selects all blocks with the model name instead of the custom display name.
`N`/`NW` retain compact export. Short display takes precedence over export. Unknown selector letters are ignored;
if none selects a block, the identification and condition header is shown.

The armor silhouette follows the original construction: bipeds at 35, 55 and 75
ton cutoffs; one quad shape; turreted and turretless ground shapes; one VTOL
shape. Destroying a turret does not switch to the turretless design. Numbers
show current front, rear and internal protection. Destroyed sections erase
their numbers and associated strokes while preserving column positions.
Two-character numeric cells retain the final two digits, matching the cockpit
layout. Colors use the reference protection thresholds; plain clients keep the
same ASCII alignment. Owned-unit diagrams show numeric values. Ordinary scans use the same strokes
and section masks with qualitative protection symbols and the three-column
reference Key legend.

The standard ASCII artwork is stored as declarative strokes, section masks and
typed numeric cells in `src/btech/status/diagrams.json`. Rust rendering uses
owned material state and existing readiness, damage, targeting and movement
queries. No C code runs, and the reference tree is unchanged. There is no copied
C template interpreter or alternate set of combat calculations. Custom reference
Lua armor-template callbacks are outside this standard-layout implementation.

Weapon and ammunition columns are independent. Ammunition is grouped by weapon
identity and ammunition mode in encounter order, sharing the compact export's
grouping and mode letters. Empty groups are omitted. The weapon side retains
stable installation numbers, section labels, rear/one-shot/ammunition/fire-mode
markers and live condition/recycling. Read-only display does not decrement
recycling, consume dice, acquire contacts or change cached heat samples.
Both weapon and limb countdowns display rounded-up two-second ticks. Disabled
ground vehicles display TRACK DESTROYED, AXLE DESTROYED or LIFT FAN DESTROYED;
a landed VTOL with rotor loss displays ROTOR DESTROYED. Concurrent vehicle and
inferno fires produce one ON FIRE banner.

The limb row also lists installed axes, swords, claws, maces and saws in reference
order, with left/right arm labels. It shows `Rdy`, a rounded two-second countdown,
or `XX` for an unusable limb. The reference display checks limb/actuator state
independently of physical-weapon component damage; actual attack admission still
uses the combat rules. Coordinate targets share the reference labels and spacing
across Mechs and vehicles. With `btech_newcharge` enabled, Mechs also show
the reference `ChargeTarget` line and elapsed `ChargeTimer` in truncated
two-second ticks in the information and short reports. Obstructed targets use
the reference line-of-sight warning. Native, Lua, gunner and observer reports
read the same host policy; rendering does not advance the timer.

The native command, Lua unit status, gunner status and observer inspection use
the same renderer. Gunners retain their own target selection. Player-authored
names are escaped before entering styled output.

## Verification

`tests/btech_status.rs` covers selectors, native/Lua agreement, damage, fuel,
ammunition, read-only state and restart. Full plain-output snapshots for all
eight silhouettes are in `tests/fixtures/btech/status/`. Review these as display
fixtures rather than regenerating them to conceal a failing test. To intentionally
refresh them after reviewing a layout change:

```sh
UPDATE_STATUS_SNAPSHOTS=1 cargo test --test btech_status status_layout_snapshots
cargo test --test btech_status
```

Broader report regression tests cover gunner selection, movement, towing,
fortification, hull-down, searchlights and literal names. Armor asset checks
verify fixed row widths when sections disappear.

A read-only comparison with the reference armor templates also verified all
256 intact/destroyed section masks for each of the eight silhouettes (2,048
comparisons). Literal strokes, numeric-cell positions and erased-section spacing
match exactly. This checks artwork independently of the Rust output snapshots;
live numeric values and damage colors are covered by the renderer tests.

An earlier focused run passed all 16 status and gunner-report tests in
`target/status-current-verification.log`. Each of the eight complete layout
snapshots is checked through both the native command and Lua. The technology
row also verifies reference ordering (TAG before supercharger and MASC), counter
color boundaries, read-only state and restart. Existing snapshot files were not
regenerated.
Both renderer unit tests and the status-export unit test also pass in
`target/status-current-unit-verification.log`;
formatting and diff checks pass.

Guardian and Angel ECM status lamps use the saved countered field: enabled
ECM is red when countered and green otherwise. Reporting does not refresh the
electronic field. The latest status/electronics/network run passes 43 tests in
`target/audit-status-ecm-countering-final.log`, including both chassis stores,
all mode/countering color combinations, live enemy ECCM and equipment destruction,
native/Lua agreement and restart. Existing silhouette snapshots remain unchanged.

STAGGERING reflects the saved action-time stagger scalar, also exposed by
read-only `StaggerDamage`. Rolling damage history alone does not activate this
banner.

## Remaining scope

This covers the currently constructed Mech, ground-vehicle and VTOL families.
Unimplemented unit families and custom player Lua armor templates require
follow-up. Ordinary scans now share standard armor artwork; their surrounding
information and weapon-table layout is still tracked separately. The broader status audit retains condition/technology details whose
behavior is not yet implemented; standard-layout acceptance does not claim
complete parity for every reference status flag.

The pre-TBITS `cargo test --no-fail-fast` run passed all 2,641 tests across 347
targets, including the complete status layout snapshots, with no failures or
ignored tests (`target/audit-post-order-integration.log`). This supersedes the
earlier failing integration baselines. It does not close the unsupported-family
and remaining behavior gaps above. See `docs/porting-audit-progress.md`.
