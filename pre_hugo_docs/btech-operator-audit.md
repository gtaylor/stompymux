# BattleTech operator interface audit

Current comparison: 2026-09-13. This is a working acceptance checklist, not a
parity declaration. The reference tree is read-only. Names alone do not prove
argument grammar, scope, authority or runtime behavior.

## Map command catalogue

Read `btmux-khi/src/btech/map/map_command_catalog.c` and the Rust native registry.
The reference has 26 map entries. All now have Rust entry points, including
ADDMINE, VIEW, @SETMAP, FIXMAP and @VIEWMAP. This is not a blanket parity claim:
FIXMAP validates derived membership, and @VIEWMAP explicitly marks the absent
C allocation counter as unavailable. Broader operator/scenario audits remain.

| Entry | Current evidence | Required work |
| --- | --- | --- |
| `@VIEWMAP` | Shared native/Lua report covers all 18 catalogue fields, ordered prefix filtering and 1/2/4-column layouts. | Delivered; firstfree is explicitly unavailable, and all 32 flag bits display safely. |
| `@SETMAP` | All 13 writable fields use one native/Lua action with bounded parsing, shared controls and rollback. | Implemented; verification and deliberate domain checks are recorded in `btech-delivery.md`. |
| `VIEW` | Shared native/Lua map-only terrain rendering, clamped coordinates, saved dimensions and labelled ANSI output. | Implemented; verification is recorded in `btech-delivery.md`. |
| `ADDMINE` | Native/Lua placement, shared newest-first insertion, persistent record identities and cross-chassis tests are implemented. | Delivered; verification is recorded in `btech-delivery.md`. |
| `FIXMAP` | Native/Lua action checks shared slot order and world invariants before reporting success. Decoder and action corruption tests distinguish rejected states. | Delivered for derived Rust membership; no separate mutable pointer index or high-water count is fabricated. |

Other names are registered: `ADDICE`, `DELICE`, `SETCOND`, `ADDBLOCK`, `ADDHEX`, `SETLINKED`, `@MAPEMIT`, `LOADMAP`, `SAVEMAP`, `SETMAPSIZE`, `LIST`, `CLEARMECHS`, `ADDFIRE`, `ADDSMOKE`, `DELOBJ`, `UPDATELINKS`, `STORES`, `ADDSTUFF`, `FIXSTUFF`, `REMOVESTUFF`, `CLEARSTUFF`.
Their individual delivery evidence is in `btech-delivery.md`; this name comparison
does not replace scenario verification.

## Mine insertion contract

`combat/mine.c::mine_command_add` accepts exactly four or five arguments. Mine
names Standard, Inferno, Command, Vibra and Trigger match in full without regard
to case. Coordinates, strength and optional extra are signed 32-bit integers;
extra defaults to zero. Coordinates must lie on the map. Stored strength clamps
to signed 16-bit, while confirmation prints the original requested strength.
The actor owns the record. New records prepend, including duplicate coordinates.

`mine_field_add` also prepends artillery-created mines, but rejects another record
at that coordinate. Rust now stores traversal order separately from persistent record slots and uses
one prepend operation for operator and artillery creation. Activation, command
detonation, tactical mine markers, listing and deletion follow that order. Auxiliary columns stay with
the original record. Tests exercise multiple kinds, clamping, native/Lua parity,
all supported chassis, artillery's no-reinforcement rule, persistence, removal,
corrupt order rejection and rollback.

## Other operator boundaries

The debug catalogue has nine entries. A `shutdown` token in the global registry
does not prove the debug-map shutdown contract; weapon recycle/BV and XP threshold
controls already exist under `@btech`, but their native forms still need mapping.
Event/memory/form/XP reports and checkpoint control require individual review.

The turret catalogue describes a separate gunner station forwarding commands to
a parent unit. Shared cockpit command names do not prove station support. Audit
initialization, gunner ownership, parent binding and forwarding authority against
the supported unit scope. Stationary vehicles and additional turret stations
are distinct concepts.

Generic special-field viewing/editing uses the reference's field catalogue and
must be reconciled with typed Rust controls. Do not equate inspection JSON with
a completed field editor, or expose unrestricted raw state mutation to make the
catalogue appear covered.

## Remaining map view and field contracts

The read-only reference checks for this section are `scripting/value_catalog.c`,
`scripting/registry_values.c`, `scripting/unit_values.c`, `map/map.c`, and
`ui/mech_tactical_map.c`. Rust candidates are `tactical_map.rs`, `viewport.rs`,
`view_preferences.rs`, `map_environment.rs`, `state.rs`, and the typed building
controls. These findings are implementation requirements, not completed features.

`VIEW X Y` requires two signed integer coordinates and clamps them to the map.
It uses the player's tactical width/height, capped by map dimensions, with row
and column labels and the player's ANSI preference. Its rendering request has
no observing unit. It shows terrain, not contact/unit overlays, and has no
cockpit, sensor-hardware, range or target-acquisition admission. Reuse the existing
hex drawing and clipping primitives; a dummy observer would introduce incorrect
rules. Tests should include tiny maps, both column parities, all edges, extreme
coordinates, preferences, ANSI, overlays, unchanged unit/contact/dice state, and
native/Lua publication rollback.

The map field catalogue contains 18 readable fields, of which 13 are writable:

| Fields | Access | Current Rust integration issue |
| --- | --- | --- |
| `buildonmap` | Read-only parent map metadata | Rust now owns `building_parent` and the saved `on_map` column. Link rebuilding sets it; BUILD removal clears it independently of LEAVE edits. The field report reads this owned metadata. |
| `firstfree` | Read-only C slot high-water diagnostic | Rust has no stored counterpart. Do not fabricate the historical counter from live unit count. |
| `mapheight`, `mapwidth`, `maxvis` | Read-only | Existing dimensions/maximum visibility are available. |
| `cf`, `cfmax`, `regen_factor` | Read/write | Existing typed building integrity/maximum/regeneration state; preserve its invariants and event policy. |
| `gravity`, `temperature` | Read/write | Reuse environment state while auditing field edits versus SETCOND's additional flag changes. |
| `maplight`, `mapvis` | Read/write | The reference uses validated light/visibility setters, not raw byte writes. |
| `mapname` | Read/write | Reference storage holds at most 29 bytes; preserve valid Rust text and distinguish display naming from asset reload. |
| `winddir`, `windspeed`, `cloudbase` | Read/write | Audit signed-short admission against canonical bearing, nonnegative wind and existing shared environmental consumers. |
| `flags`, `sensorflags` | Read/write | Support deliberate bit-vector editing without bypassing typed state validation. |

Field names match in full, case-insensitively. The setter parses a field name
plus remaining value text; successful field writes are silent. Read-only or
unknown fields reject. The viewer has optional `1` or `4` column selection,
otherwise two columns, followed by a case-insensitive field-name prefix filter.
Fields appear in catalogue order. Its title identifies the current object and
special type. Do not mistake the viewer's prefix filter for setter abbreviation.

Raw numeric field widths matter: source char/short writes first parse a signed
32-bit number and clamp to their storage width. Map light/visibility have their
own range checks. Integer fields parse signed 32-bit. Bit-vector input accepts a
signed integer or a letter sequence with `!` clearing the following letter from
the newly constructed value, rather than editing the previous stored value.
The source has unsafe shifts for letters beyond the integer width; those must
not be reproduced as undefined behavior. Explicit tests must settle valid bit
width, negation, zero formatting and invalid input behavior.

`FIXMAP` is about reconciling location, unit membership and map-slot indices. It
is not ordinary shutdown or unit removal. The reference removes invalid/wrong-map
slot entries, clears stale placement claims for contained units absent from the
index, and reports mismatched slots. Rust's validation prevents several of these
states from loading. Its administrative contract needs to be defined against
representable state and the existing shared membership rules before adding a
command that would merely print a success message.

## Debug entry-point contracts checked after map delivery

Read-only evidence: `integration/debug_command_catalog.c`, `integration/debug.c`,
`character/character_persistence.c`, `unit/mech_partnames.c`; Rust candidates:
`commands.rs`, `weapon_settings.rs`, `map_clear.rs` and character skill controls.

- SETVRT takes exactly weapon/value, parses signed 32-bit, accepts 1–127,
  resolves an exact very-long part name and requires a weapon. It publishes
  the selected catalogue name/value and records a wizard change. Existing Rust
  controls enforce the value range. Standalone SETVRT and SETWBV now share the
  existing @btech edit/confirmation path. VRT edits now share commit-staged
  WIZ/CHANGE records across standalone, @btech and Lua actions.
- SETWBV has the same weapon/value grammar, accepts nonnegative signed 32-bit,
  and publishes the selected value. Its catalogue lacks the `@` permission marker,
  unlike the other mutators; inspect actual dispatch authority before deciding
  whether that omission is intentional. Shared Rust controls currently require
  wizard authority.
- SETXPLEVEL takes exactly skill/value, accepts nonnegative signed 32-bit, rejects
  non-skill character values, and logs a silent successful threshold change.
  Zero disables gains. Standalone SETXPLEVEL now delegates to the existing
  skill-threshold control, with exact two-argument syntax and silent success.
  @btech retains name=value and its confirmation. Native and Lua threshold edits
  now stage canonical WIZ/CHANGE records through the same action; disabled wizard
  logging omits the record, while failed transactions discard it.
- SHUTDOWN takes a numeric map ID, shuts down each indexed unit, resets position
  and removes membership, then clears dynamic map indexing. It is distinct from
  the cockpit shutdown form. Rust now dispatches a numeric argument to the
  shared wizard-only bulk clear action, while bare SHUTDOWN retains cockpit
  behavior. Cross-chassis lifecycle comparisons and authority checks pass.
- SAVEDB requests a normal SQLite checkpoint and reports its actual success or
  failure. Rust now stages a forced-save effect through the existing serialized
  commit path, with native/Lua requests, post-persistence confirmation and a
  real locked-database failure/retry test.
- LISTFORMS iterates the full part registry in short-name order and publishes
  short, long and very-long forms. Rust now retains these forms in the shared
  inventory name index, reports them through paced output and exposes detached
  Lua inspection. Catalogue identities, manufacturer collisions, authority and
  complete TCP delivery are covered by acceptance tests.
- XPTOP requires a skill, excludes wizard players and zero raw XP, ranks remaining
  XP modulo XP_MAX descending, displays at most 16 entries and shares of the
  counted total. Rust XPTOP and Lua now use the shared low-24-bit balance,
  widened totals, deterministic database-order ties and zero percentages when
  the total is zero. The first 10,000 eligible players are counted before ranking.
- EVENTSTATS and MEMSTATS still need runtime-specific report mapping; do not
  fabricate C allocation/event counts for Rust data structures.

Follow-up dispatch evidence: `special/registry.c::handled_command_sub` only applies
special-command privilege checks when the help text begins with `@`. SETWBV's
missing marker therefore bypasses that reference guard; this is a real discrepancy
with the wizard-only shared Rust setter, not just a help-format issue.

Separate gunner stations (`movement/ds_turret.c`) accept any registered parent
Mech object without a dropship-class guard. The filename alone does not exclude
supported ground/VTOL parents. Forwarded actions require the registered gunner
and reject simultaneous parent piloting. Each station owns targeting coordinates,
lock modes and arc selection; the reference temporarily overrides combat pilot,
arcs and targeting while calling shared parent actions, then restores parent
state. Rust should use an explicit action context if implementing this, rather
than global mutable overrides or duplicate firing code. INITIALIZE allows takeover
when the previous gunner is disconnected or elsewhere; DEINITIALIZE requires the
current gunner. Five TIC forwarding functions are empty in the reference, so
command catalogue presence is not functioning TIC evidence. Rust now owns saved
station fields and provides registration, independent occupant assignment, takeover,
release, inspection and explicit admission context. Supported-parent lifecycle and
selective persistence are tested. Shared native/Lua target selection and independent lock progression are now
implemented. `combat/mech_combat.c` skips EVENT_LOCK when
`core/context.c::btech_context_overrides_weapon_arcs` sees a nonzero arc mask;
Rust matches immediate locks for that case and ordinary settling for zero masks.
Combat/report forwarding, firing arc application and native field editing remain
acceptance work.


The very-long lookup was checked through `unit/mech_partnames.c::part_match_next`:
PART_MATCH_VERY_LONG dispatches to exact hash lookup, while PART_MATCH_LONG uses
wildcard iteration. SETVRT/SETWBV use the former. Their standalone Rust commands
therefore retain exact canonical/manufacturer names and reject wildcards. Both
are wizard-only, matching the existing shared Rust mutation policy; the SETWBV
reference permission omission is deliberately not exposed as a global mutator.

Gunner skill follow-up: `unit/mech_lifecycle.c::mech_has_active_gunner` uses the
connected override operator, independently of the parent pilot. Rust's explicit
gunner context now supports that skill lookup and rejects stale station identity.
`unit/mech_identity.c::find_gunnery_skill_name` distinguishes non-extended VTOL
Aerospace gunnery from ground-vehicle Conventional gunnery; shared Rust hit/XP
selection now follows that distinction. Direct-fire admission and XP eligibility
now recognize station operators without broadening ordinary pilot controls.

### Station equipment inspection

WEAPONSPECS, WEAPONSTATUS and CRITSTATUS share parent-unit report builders after
station ownership admission. Report authority accepts the registered gunner
independently of ordinary cockpit pilot controls. Seven-chassis tests verify
matching pilot/gunner text, release denial, passenger restrictions and the
separate gunner-consciousness/parent-blindness checks. Twenty focused checks pass
(`target/gunner-reports-final-tests.log`). Broader station report and combat
forwarding remain incomplete.

### Explicit station selection in combat calculations

Shared target decoding, observer selection, lock penalties and terrain-purpose
lookup now accept a physical unit plus a selection owner. `BattleGunnerContext::aim`
and Lua `btech.gunner.aim` use this with registered station ownership and explicit
gunner skill. They preview conventional unit/coordinate aim without modifying
parent state or dice. Station unit locks and parent unit locks independently
override the parent's datalink; mixed-class LRM/MML and empty-coordinate observer
checks cover both cases. MML short-range mode still rejects a selected observer.

Preview remains read-only. Live direct fire now uses these shared calculations;
native station SIGHT and artillery are covered below.
The 57 focused aim, sighting, target, artillery, special-ammunition and
vehicle checks passed (`target/gunner-target-source-tests.log`); the expanded
station/MML/empty-coordinate cases passed in the final nine-check run
(`target/gunner-target-source-final-tests.log`).


### Live gunner direct firing

FIRE and Lua `btech.gunner.fire` now use the physical parent's existing launch and
damage paths with independently admitted station ownership, selection, skill,
arc mask and XP recipient. Cockpit audiences are retained and station audiences
receive the same firing feedback. World/effect checkpoints include those added
notices. Shooter injury and recoil retain the physical pilot's toughness;
movement, radio and shutdown authority remain pilot-only.

The 438-check combat regression run passed, followed by 21 station/report/help
checks. WEAPONS shares station inspection and exposes the parent's readiness. Actual Mech/vehicle/VTOL damage awards XP only to
the gunner. Classic award tests additionally verify all seven chassis and
connected/present station eligibility. Final admission sharing was checked
against station fire, TIC and weapon-power behavior. See the `gunner-fire-*`
logs in target. Native SIGHT, broader operator
commands and final integration scenarios remain unfinished.


### Station terrain firing

Station FIRE now admits terrain through shared hex-shot/coordinate-launch code.
Operator-owned mode, skill and observer selection remain independent of parent
locks, while parent equipment owns expenditure and terrain consequences. The
same mask checker serves unit bearings and coordinate centers. Source injury
and recoil continue using physical-pilot toughness.

The 162-check run (`target/gunner-terrain-final-tests.log`) passed. Station tests
cover every supported chassis and terrain purpose, parent comparison, native/Lua,
rollback, restart and rejected arcs. Mixed LRM/MML observer shots preserve a
conflicting parent lock while firing at the observer's empty coordinate.
Station artillery now carries explicit ownership through preparation, correction
and delayed impact, as covered below. Native SIGHT and broader integration acceptance remain open.


### Station artillery ownership

Live station artillery uses shared preparation, launch expenditure and delayed
impact. Selection, skill, arcs and correction belong to the station; the physical
unit remains the shooter. Queue persistence retains both identities. Missing or
reassigned stations receive no correction, while their admitted shells still
arrive. Correction persistence participates in transactional save and purge.

All supported chassis have live launch coverage. Station tests additionally check
independent correction, corrected subsequent aim, restart, failed publication/save,
station removal with a shell in flight, and observer/datalink invalidation.
Native station SIGHT is covered below; wider operator/scenario acceptance remains open.

Verification: all 2,230 tests passed across 274 result targets in
`target/gunner-artillery-full-tests.log`. This is the current full-suite baseline
and includes station ownership, targeting, skills, direct/terrain/artillery fire
and the new observer-invalidation checks. The earlier focused artillery run
passed 48 tests (`target/gunner-artillery-final-tests.log`).
All-target Clippy with warnings denied passed (`target/gunner-artillery-clippy.log`),
as did formatting, diff checks and the Lua declaration mirror comparison.
The reference tree remains unchanged.


### Station SIGHT

Native SIGHT and Lua `btech.gunner.sight` now share the cockpit sight action with
explicit physical-unit and selection-owner identities. Station skill, unit/hex
selection, observer override and arc masks pass through the common calculations.
Artillery uses the same station-aware preparation as live launch. Results go to
the station operator; the parent's selection and pilot assignment are unchanged.

Sighting requires healthy, running controls, but permits weapons hold, empty
ammunition and recycling. It commits only preparation/sensor/attack dice and
notices, never heat, expenditure, damage, correction or queued shells. Callback
and publication failures restore both dice and notices. Redundant cockpit-only
target, safety, observer and dice adapters were removed after all callers moved
to the shared explicit-source functions.

Acceptance covers seven chassis, conventional weapon families, selected and
explicit unit/hex targets, independent skill/selection/masks, artillery, observer
and MML mode changes, denied authority, rollback and restart. Broader station
reports, field controls and final integration scenarios remain open.

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

STATUS resolves registered station authority before rendering parent state with
the station's independent selection. Mechs and vehicles share targeting display
logic. Lua exposes the guarded `btech.gunner.status` entry point; the global unit
inspector remains unchanged. Tests cover seven chassis, all sections, native/Lua
equality, blindness/unconsciousness, release, passengers, shutdown and restart.
CONTACTS and field controls still require station acceptance.

## Station CONTACTS

CONTACTS now shares an occupant report service for physical sensor visibility,
brief formatting and ordered rows while retaining station selection and viewer
preferences. Guarded `btech.gunner.contacts` returns the native formatted report.
Building identification retains original station/parent identity across callbacks;
actor movement, parent reassignment and station removal cancel with rollback.
Seven-chassis acceptance covers independent selection, native/Lua output, saved
preferences/restart, unavailable contacts, denied/invisible structures and callback
authority changes. All 90 contact/scan/report/help checks passed. Station field
controls and final integration acceptance remain outstanding.

## Station wizard fields

SETTURRET and VIEWTURRET are implemented with wizard-only native dispatch and Lua
field operations. All eight fields map to owned station data. Coordinate fields
use independent signed-short values, correcting the reference's target-ID alias.
Exact numeric edits preserve deferred setup references; ordinary gunner admission
still rejects unsupported/unavailable parents. Map/station listings share their
prefix parser and column renderer. Acceptance covers native/Lua equivalence,
permission checks, integer limits, state isolation, rollback and persistence.
EVENTSTATS/MEMSTATS and the final command/scenario audit remain outstanding.

Follow-up finding: the reference station catalogue includes ADDTIC, DELTIC,
CLEARTIC, LISTTIC and FIRETIC. Rust `tic::controlled` still admits only the physical
pilot and native TIC commands pass the occupied station ID directly. Station TIC
control therefore remains an explicit implementation gap for the next pass; the
completed report/field work does not establish full station command coverage.

## Station TIC fidelity correction

Direct inspection of `movement/ds_turret.c` shows ADDTIC, DELTIC, CLEARTIC,
LISTTIC and FIRETIC are empty handlers. Their catalogue entries are not evidence
of working station group controls. The prior claimed implementation gap was
incorrect. Native station dispatch now preserves these inert operations, including
ignored arguments and uninitialized stations; parent cockpit TICs retain their
existing authority and behavior. Seven-chassis tests prove no changes to weapon
groups, selection, dice or simulation state. No station Lua TIC facade is added.

## Runtime diagnostics

EVENTSTATS and MEMSTATS now expose actual Rust measurements. The server and report
share one simulation-work predicate; reactor startup grace is explicitly reported.
Counts include scanner observers, queued artillery and settling station locks.
MEMSTATS reports live record counts, inline root/map/unit/station sizes with heap
storage explicitly excluded, and exact compact JSON bytes using a counting writer.
LONG adds registration counts. Allocator totals remain explicitly unavailable,
not estimates or fabricated C allocation counts. Guarded `btech.runtime.stats`
returns the same detached snapshot. This supplies the runtime-specific mapping;
full cross-command/scenario acceptance remains open.

## Cockpit catalogue follow-up

Actual handler review confirms additional work beyond runtime reports. The HEAT
entry calls the existing flamer heat control; Rust now accepts HEAT as an alias
without duplicating selection or firing-mode logic. SETTEAM calls a real wizard
team setter; Rust now exposes native and Lua actions using the shared sensor
signature setter and its C3/C3i invalidation.

Still requiring implementation/acceptance: @OOD, @LOSEMIT, @DAMAGE,
@DAMAGESECTION, @WEIGHT, SETMAPINDX, SETXY, @SETMECH, @VIEWMECH,
@SETSPECIAL and @VIEWSPECIAL. Names absent from the native registry do not alone
prove domain logic is absent; reuse existing typed controls when adding these
interfaces. SNIPE is a manual wizard artillery predictor implemented in an
autopilot source file, so the filename alone does not establish its exclusion.
Its behavior and relationship to the excluded autopilot need explicit audit.
THRASH attacks battle suits, and CHECKLZ is aerospace-specific; these belong to
the excluded unit scope. Other unmatched catalogue tokens include headings and
excluded repair, passenger-loading and unit-type operations; continue validating
handlers individually instead of treating a textual token count as completion.

## @LOSEMIT

Read `sensors/mech_los.c::mech_losemit` and `ui/mech_notify.c::mech_los_broadcast`.
The operation requires a conscious actor and mapped source, not a running source;
recipients must be started and see the source. Source exclusion, identification
prefixes, apostrophe spacing and private completion confirmation are now provided
through the existing Rust observer and notification paths. Native and Lua actions
share authority and atomic output, with seven-chassis and restart acceptance.
@LOSEMIT is delivered; the other scenario/field entries listed above remain open.

## @DAMAGESECTION

Located wizard damage is implemented using shared material, critical, injury and
evacuation actions. Numeric bounds/nonzero flags and chassis section parsing are
covered through native/Lua acceptance. Rear vehicle hits redirect front to rear
and retain their extra diagnostic roll; self attribution is preserved. Tests prove
actual armor loss, lethal destruction, rollback and restart across seven chassis.
The separate @DAMAGE handler is not complete: direct handler inspection shows its
missile request uses clustersize as packet count and damage/clustersize as packet
size, discarding the remainder. Do not substitute a conventional cluster split
based solely on the catalogue description.


## Target chassis and hit arcs

The `@DAMAGE` trace found and corrected a common combat defect before command
integration: ground vehicles and VTOLs now use their configured hit-arc widths,
selected by the target rather than the attacker. Shared material packet routing,
vehicle beacon attachment, shot admission and front-only dug-in cover consume
that classification. Mode-zero vehicle front coverage is 60 degrees; modes one
and two use 90 and 180 degrees respectively. This correction does not close the
outstanding wizard command or whole-integration acceptance requirements.


## Wizard packet damage delivery

`@DAMAGE` is now delivered with its native four-integer grammar and a guarded Lua
counterpart. The reference's cluster-count/truncation behavior, random critical
selection and persistent rear flag are retained. All supported chassis use the
shared material packet and casualty paths with rollback and durable dice.
Unplaced units are admitted; self-hit cover uses current geometry rather than
a stale flag from another observer. `@OOD`, `@WEIGHT`, `SETMAPINDX`, `SETXY`,
`@SETMECH`, `@VIEWMECH`, `@SETSPECIAL` and `@VIEWSPECIAL` remain outside this
completed command slice; their behavior and the broader integration gates still
require implementation or verification.


## Wizard weight allocation delivery

`@WEIGHT` is delivered through the shared construction-mass services, with a
private common native/Lua report and exported typed rows. It counts original
components and installed ammunition-bin capacity rather than damaged material
or current supplies. Permission, unplaced designs, damaged/depleted units,
restart, literal text and output rollback are covered by focused acceptance.
The remaining unit scenario/field commands and full integration audit are still
open; this entry supersedes the earlier list's missing `@WEIGHT` item.


## SETXY delivery

`SETXY` now supports its two/three-integer grammar, explicit signed-short altitude,
running and stopped supported units, flight/descent continuation and private
confirmation. Incoming targeting observations are cleared while outgoing selection
and controls remain. Guarded Lua shares the action and rollback. Attached tow
partners relocate together to preserve relationship consistency. `SETMAPINDX`,
`@OOD` and unit field interfaces remain open, along with the full integration
completion audit; no readiness claim follows from this command's delivery.


## Battlefield identity prerequisite

The Rust identity service now persists assigned IDs independently of membership
slots and prevents collisions across Mechs and vehicles. It has no cockpit or
Lua command surface yet. `SETMAPINDX` remains open, including reassignment and
removal behavior; this prerequisite does not change the integration readiness
status.


## Scenario reassignment destination service

The trusted Rust destination service now covers live running/stopped units,
independent ID assignment, destination capacity, origin reset, preserved controls,
airborne continuation on compatible maps, and tow release. Native/Lua SETMAPINDX
and removal lifecycle are still open. No command-completion or whole-integration
claim follows from this service delivery.


## SETMAPINDX host and removal delivery

SETMAPINDX and guarded Lua setmapindex now support assignment and -1 removal,
private confirmations and atomic rollback. Detached membership is saved separately
from the retained pose; immediate re-entry preserves controls, while the next
update performs shared shutdown cleanup. This supersedes the earlier missing
host/removal implementation entries. Configured preferred-ID behavior and jump
routes incompatible with a destination still require follow-up before full
reference parity can be claimed. Other scenario/field commands and whole-goal
acceptance remain open.


## Configured identity selection

Constructed Rust units now retain a validated preferred ID independently of
the active battlefield label. Guarded Lua can set or clear it, and SETMAPINDX
uses it when the explicit ID is omitted or too short. Explicit overrides and
collision handling leave the preference intact. This supersedes the earlier
missing configured-ID behavior item for Rust-owned construction. Reference-state
import and full integration acceptance remain separate audit requirements;
destination-incompatible jump routes are still open.


## Reassigned airborne routes

SETMAPINDX now preserves a jump across smaller maps and changed wrapping policy.
Future samples use the destination boundary rules; ordinary edges clamp and use
the shared landing service. Biped/quad restart and native/Lua rollback tests cover
this route-continuation slice. Earlier incompatible-route implementation gaps
for ordinary boundaries are superseded. This does not complete the broader
scenario/field-command or whole-integration audit.


## Orbital insertion and combat

`@OOD` and guarded Lua `btech.unit.ood` now insert all supported ground chassis
and VTOLs. Ground drops retain mass-based cocoons, share the descent/landing rules,
and hand breaches to jump-jet compensation or the existing forced-fall clock.
VTOLs use ordinary flight state. Shared damage entries, launch handlers and aim
modifiers now apply cocoon interception, firing-induced opening and the intact
protection targeting bonus. These supersede the earlier missing @OOD entry;
landing callback delivery (`on_ood_land`) now runs before the landing roll and
material consequences, with whole-tick rollback. Broader field-command/construction
acceptance remains open.


## Common field command follow-up

The next remaining native interfaces are @SETMECH/@VIEWMECH and
@SETSPECIAL/@VIEWSPECIAL. The reference routes all four through the same field
service (`scripting/registry_values.c`, `scripting/value_catalog.c`), selecting
the actor's current object's type. Rust already has map and gunner field actions
and a shared `field_report` renderer. Unit fields need their own typed catalogue
and projections over existing unit services; the common native dispatch should
reuse those actions rather than implement setters separately for each command
or chassis. Read-only fields, named damage encoding, live status/critical flags,
construction attributes, geometry, crew and targeting need explicit coverage.
These commands are not yet implemented; excluded repair, carried-unit and unit-
class features remain outside the goal.


## Initial common field implementation

@VIEWMECH and Lua unit.fields now share a catalogue and initial live projections.
@SETMECH/unit.set_field implement team and xpmod, with typed validation and atomic
rollback. @SETSPECIAL/@VIEWSPECIAL reuse map, station and unit field actions.
The remaining unit-field reads are explicitly unavailable; all other unit setters
are still open. In particular, do not treat the new command registrations as proof
that the status/critical masks, construction fields, damage encoding, identity,
crew/targeting mutation or complete field contract have been implemented.
