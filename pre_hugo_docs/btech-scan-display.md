# Scan display parity

Ordinary unit scans now use the standard status armor artwork for bipeds,
quads, ground vehicles and VTOLs. One renderer owns anatomy selection, section
masks, protection thresholds and fixed-width cells. A scan fills each cell with
`O`, `o`, `x`, `X` or `*` and prefixes the reference three-column Key legend.
Legend row placement follows each silhouette; destroyed sections erase the same
strokes and cells as owned status. Armor and internals are never emitted as
numbers in this mode. Privileged observer scans retain owned numeric status.

The reference contract is `ui/mech_status_armor.c` and the standard artwork in
`ui/mech_status_armor_templates.c`. The original integer damage bands and zero
special case determine both color and symbol. The legend runs from open to
healthy protection; it does not add percentage prose to the layout.

`tests/btech_scan_diagrams.rs` exercises all eight silhouettes through native and
Lua scans, detached read-only reports and restart. It verifies numeric disclosure
boundaries, legend entries and the three-column width difference from owned
status. Existing full cockpit snapshots are unchanged. The scan, mixed vehicle
scan, gunner scan, diagram and status suites pass 94 tests in
`target/audit-scan-diagrams.log`.

Ordinary scan weapon tables now use one shared renderer with the reference
header, 18-column weapon names, two-character display numbers, 14-column
locations and five-character status. It strips technology prefixes and omits
ammunition. Destroyed sections are omitted; display numbers compact without
changing the underlying mount indexes or recycle clocks. The first critical
controls the public damage marker independently of complete firing admission.
The table reads current clocks without advancing them. Native and Lua continue
to share transactional scan warnings.

The three tests in `tests/btech_scan_weapons.rs` cover all seven chassis,
empty-ammunition readiness, zero/nonzero recycle boundaries, first versus later
critical loss, exact columns, destroyed-section numbering, native/Lua output,
read-only state and restart. They pass in
`target/audit-scan-weapon-columns-final.log`; the other 94 scan/status tests pass
in `target/audit-scan-weapons.log`. All-target Clippy passes in
`target/audit-scan-weapons-clippy.log`. A broader full-suite run is in progress
in `target/audit-scan-display-full-suite.log`; it is not a completed verification.

Remaining work includes repairing-state `?`, unsupported unit families and
custom player templates. Those are separate from standard armor silhouette
acceptance and remain in the main porting audit.

The shared report block follows
`sensors/mech_scan.c::mech_scan_print_report`. Its identity row has a
25-character display-name field. Following rows use six-space indentation,
tab-separated range/bearing and speed/heading, three-column coordinates and
heat multiplied by ten. Heading includes lateral motion; flying types add
vertical speed. Turret facing, full movement names, turret/weapon arcs, detail-1
condition banners and jump heading follow, then a single-space separator. The
INFO supplement adds torso-left/right and towing lines. `scan_summary.rs` reads existing motion and facing queries. Arc descriptions,
turret geometry and turret formatting are shared with contact/weapon/cockpit
reports. Condition predicates are shared with status, with cockpit-only
electronics, equipment and control details excluded from ordinary reports.
Inspection remains read-only.


`tests/btech_scan_summary.rs` verifies all seven chassis, the fixed name field
and literal markup, indentation, tabs, movement names, lateral heading,
positive-180 and wrapped turret offsets, stationary offsets, both observer arcs,
VTOL vertical speed, jump heading, visible condition disclosure, torso INFO and
mixed-family towing. Native and Lua reports agree and survive restart.
All seven scenarios pass in `target/audit-scan-summary-layout-final.log`.
The 71 scan regressions, 230 library tests and two vehicle-weapon regressions
pass in `target/audit-scan-summary-final.log`. Other affected scan/status suites
passed in `target/audit-scan-summary.log`; its one obsolete broad “Weapon”
substring assertion was corrected to exclude the weapon table while permitting
the required Weapons Arc line. Current all-target Clippy passes in
`target/audit-scan-summary-clippy.log`.

The full-suite process started before this report change and is still running.
It is baseline evidence for the earlier scan armor/weapon work, not full-suite
acceptance of the newer information rows.
