# Lua API contracts

The C bindings under `btmux-khi/src/mux/lua/packages/mux` define the non-BattleTech
API. The checked-in [callable inventory](../tests/fixtures/lua-api.json) records
functions and methods with their C source references. Integration tests resolve
every entry against initialized bindings. Existing package-specific tests cover
flags/powers, state, communication, text, flows, testing and configuration.

## Packages and identities

`require("mux")` returns the global `mux` namespace. Built-in bindings initialize
before editable game modules. The packages are `world`, `session`, `config`,
`text`, `comsys`, `telnet` and `error`; `log` and `check_db` are top-level functions.
`require("btech")` also exposes the map and inspection subset documented in [BattleTech](btech.md). Unit construction, administrative placement, cockpit assignment, startup/shutdown, ground motion controls, contacts/target selection, conventional tactical firing, weapon readiness and detached character-health inspection are available. Full combat and map operations beyond explicit creation/reload remain deferred.

Objects are immutable userdata identified by world, dbref and incarnation.
`mux.world.object(dbref_or_object)` creates a handle; `:dbref()` retrieves its
identity. A table with an `_id` field is not an object. Two handles for the same
live incarnation compare equal. A handle retained across rolled-back creation
cannot address a later object allocated at that provisional dbref. Object, state,
flag, power and channel handles reject stale identities after destruction.

Typed catalogs remain immutable: `world.types`, `world.flags`, `world.powers`,
`world.locks` and `comsys.flags`. Constants from different catalogs are not
interchangeable. Command/configuration aliases do not change Lua constant names.
Host closures, module lookup tables and connection snapshots are private. Returned
session tables are fresh values; editing them cannot change connection state.

## Structured errors

```lua
local errors = mux.error.namespace("mygame", {"access.denied", "input.invalid"})
local ok, result = mux.error.pcall(function()
    mux.error.raise(errors.access.denied, "Access denied", {door = 13})
end)
assert(not ok and result:is("mygame.access"))
local outer = mux.error.wrap(result, "mygame.command", "Command failed")
assert(outer:root() == result)
```

`new{code, message, detail?, cause?}` constructs a value; `raise` throws one.
`is(value, code)` and `error:is(code)` use exact or dotted-prefix matches.
`check(value, failure)` returns a truthy value unchanged or raises the supplied
failure unchanged. `wrap` preserves structured causes and normalizes other errors
as `mux.runtime`. `root` follows table causes with bounded, cycle-safe traversal.
`mux.error.pcall` retains all successful return values, including intervening nils;
failures retain structured table identity and receive traceback information.

`mux.error.codes` is `code_tree("mux")`. The other native roots are `testing` and
`btech`; BattleTech inspection uses its template and operation error symbols.
Custom namespaces use dotted lowercase segments and cannot use those roots.
Code nodes support `.code`, child lookup, equality and string conversion. Unknown
symbols and invalid namespace declarations fail explicitly.

Domain failures travel as typed host errors and become structured Lua values at
binding/protected-call boundaries. Ordinary `pcall` and `xpcall` can inspect their
codes. Arbitrary Lua error values remain intact; ordinary argument conversion/type
errors remain ordinary errors. Error messages include context, but callers should
branch on codes rather than parse messages. The complete native code catalog is
in `src/lua/packages/error/catalog.rs`.

## Movement, destruction and repair

`mux.world.teleport_object{object=..., destination=...}` uses shared movement
policy, containment validation, locks and object callbacks with GOD as cause and no
invented descriptor. Location transition providers/events use C cause `#-1`.
Teleport-source and leave actions precede relocation; appearance precedes
teleport/move and enter actions. DARK teleports retain silent location providers
and move events, while suppressing teleport and location events. Only players and things can move; destinations must be valid
containers. A same-location move is a no-op. Denial raises an error, and callback
failure restores movement and staged effects even when caught with `pcall`.

Native generic movement also renders immediately after relocation. Exit traversal
runs exit success/on_success, source leave/on_leave and destination enter_source,
relocation/appearance, exit drop/on_drop, traveler move/on_move, destination
enter/on_enter and source leave_destination. Exit actions use `operation="traverse"`;
traveler/location actions use `"move"`, retaining the original command cause.
That cause retention is an approved Rust difference: C's normal exit/enter/leave
command entrypoints supply `#-1` to movement actions (see parity finding M05).
Providers still execute when silent, with direct messages retained and neighbor
messages/events suppressed according to the action policy. Shared action neighbor
text uses the bounded notification graph, including AUDIBLE forwarding.

`mux.world.destroy_object(object, {override=true})` silently schedules normal
GOING destruction. Options are optional; `override` bypasses SAFE, not protection
of foundational objects or Wizard players. Scheduling does not immediately purge.

`mux.check_db()` performs semantic repair synchronously, using the same planner
as `@dbck`. Later Lua statements immediately see repaired objects and tombstones.
Relocation callbacks execute in that boundary. Owned-record cleanup, SQL integrity
checks and disconnects happen only after the outer world transaction commits.
A failed save restores the prior world and discards messages, logs, flows and
maintenance effects. Multiple checks retain pending cleanup until commit.

Live test invocations keep their established semantics: valid mutations can commit
even after an assertion/runtime error; invalid or unsaved invocations roll back.
Checking VMs permit pure configuration/text/error helpers and constants, and raise
`mux.unavailable.checking` for live services. State enumeration/mutation, repair,
teleportation, destruction and staged logging require an active transaction.

## Telnet environment

```lua
local term = mux.telnet.environment_get(ctx.descriptor, "var", "TERM")
local enabled = mux.telnet.environment_has(ctx.descriptor, "uservar", "CLIENT_FEATURE")
```

Both calls require a live session ID, including a live unauthenticated connection
when explicitly supplied. Namespace names are exactly `var` or `uservar`.
Names and values are byte-preserving Lua strings. `get` returns nil for an absent
entry; `has` distinguishes absence from an empty value. These read-only lookups
use the latest host snapshot and cannot alter negotiation or persistent state.

## Intentional Rust behavior

BattleTech `unit.jump(unit, pilot, bearing, range)` shares the native dry-terrain/water/ice
jump implementation and transactional launch notification. Unit state exposes
detached flight progress, airborne coordinates and the stabilization countdown.
Conventional airborne fire uses the shared damage resolver, including gyro and
jet falls and structural collapse. Ordinary hill collisions and elevation changes are supported; special-terrain landings, DFA
and remaining jump combat interactions remain
unsupported; see `btech.md` for the current gameplay contract.

- Markdown documents and effective/defaulted TOML configuration lookup remain
  supported extensions. Strings keep existing bracket-markup behavior.
- Existing configured instruction, memory, state and output budgets apply.
- World/output/flow/maintenance effects participate in nested rollback; Lua file
  logs are submitted only after commit. Logging-only calls do not write the DB.
- CONNECTED remains session-owned and is never writable through flags or snapshots.
- Native Lua services use shared domain logic and asynchronous outer persistence;
  they do not call SQLite synchronously from the VM.
- No C internals, general host filesystem access or extra debug/native module
  loading is exposed. BattleTech asset inspection reads bounded named files
  confined to the configured mech/map directories.

`mux.text.is_printable_ascii(value)` requires an actual Lua string and raises an
argument error for all other types. It checks bytes `0x20–0x7e` without UTF-8
conversion; empty strings return true, and embedded NUL/non-ASCII bytes return false.

`btech.unit.land(unit, pilot)` shares native `land` control and landing rules.
It returns true for a completed attempt, including a failed control check that
causes a fall; invalid requests raise a structured operation error. Callback
rollback restores flight, dice, damage and notifications.
For VTOLs, it attempts touchdown or cancels a queued launch. Aircraft use the
shared live vehicle lifecycle and host consequence transaction.

`btech.unit.enterbase(unit, pilot, direction)` shares native `enterbase` parsing,
admission, enter-lock callbacks and notice staging. It returns true when the
18-second event is scheduled, false for lock denial, and a structured operation
error for invalid eligibility or arguments. Callback rollback removes the event
and staged notices. The server rechecks the current route and locks at expiry.

`btech.unit.takeoff(unit, pilot, delay)` shares native `takeoff` checks and notice
staging; nonzero delay overrides require a wizard. `btech.unit.vertical(unit,
pilot, speed)` shares native vertical control, returning true after a change or
the current speed when omitted. Both use configured fuel rules and callback
rollback; invalid requests raise structured operation errors.

`btech.unit.stand(unit, pilot, mode)` shares native stand rules and notice staging.
The optional mode is `normal`, `anyway` or `careful`. It returns the skill check,
optional fall report and rise/retry timer, with callback rollback covering ice
fracture and neighboring casualties. Standing beneath intact ice preserves bottom altitude.


`btech.unit.auto_fall(unit, pilot, enabled)` shares the `mechprefs AutoFall`
mutation and cockpit checks. It returns true, participates in callback rollback,
and permits a stopped engine. `btech.unit.state(unit).auto_fall` is a detached
boolean snapshot of the durable preference.


`btech.unit.lbx(unit, pilot, weapon)` toggles an intact, recycled LB-X between
`normal` and `cluster`, using the native `lbx` guards and cockpit notices.
Mode changes and notices roll back with the callback. Weapon inspections and
shot expenditure expose `ammunition_mode`; loadout mounts expose
`initial_ammunition_mode`, and ammunition bins expose `mode`. Shot reports expose
`aim.ammunition_accuracy` (−1 for cluster). Selected modes survive restart.

Rocket and one-shot missile launchers use the existing `btech.unit.fire` call.
Weapon inspections expose `one_shot` and `readiness.spent`; loadouts expose
`one_shot` and `initially_spent`. Readiness ammunition is the mount's remaining
self-contained salvo, and shot expenditure has no external ammunition bin.
Failed Streak locks preserve this salvo. Callback rollback restores expenditure,
and committed spent state survives recycle completion and restart.

Installed targeting computers contribute `aim.targeting_computer` to the
`btech.unit.fire` result: −1 for assisted direct fire, otherwise zero. Assistance
is derived from installed slots and current damage/flooding, persists through
restart, and shares native firing and callback rollback behavior.

`btech.unit.state(unit)` exposes `gyro` (`standard` or `hardened`) and
`gyro_damage`, the effective stability damage after hardened protection.
Physical gyro losses remain available through critical inspection. Movement,
standing and jumping share this derived damage and preserve it across restart.

Ammunition loadouts expose `capacity` and `half_ton` for explicit `Halfton` bins.
`rounds` describes initial template contents; live ammunition and weapon
readiness report remaining salvos. Firing and rollback use the ordinary shared
ammunition path, including half-ton LB-X cluster bins.


Native unit creation and `btech.unit.create` infer ammunition bin size from the
authored quantity and normalize it to the selected capacity. The owned unit
definition contains the normalized initial quantities; source assets remain
unchanged. Saved live ammunition loads independently and is never normalized
or refilled during restart. Unsupported flags still reject construction.


`btech.template.check(name)` returns read-only construction diagnostics shared
with native `@btech template-check`: constructibility, first rejection reason,
resolved counts and ammunition normalization adjustments. It follows the
existing template asset bounds and callback requirement, returns detached data,
and does not register a unit or mutate world state. Counts are meaningful on
successful construction; rejected reports contain zero counts.

Artemis, hotload and timed unjam controls now share the native transaction paths
through `btech.unit.artemis`, `btech.unit.hotload` and `btech.unit.unjam`.
Detached unit/weapon inspection exposes controller links, jam state, mode and
recovery countdown; shot reports distinguish hotload jams from failed launches.

`btech.unit.rottorso(unit, pilot, direction)` and
`btech.unit.fliparms(unit, pilot)` share native facing controls and cockpit output.
They require a transaction and the assigned conscious pilot; torso directions are
case-insensitive left/right/center or l/r/c. Both return true on success. Native/Lua
pose/output parity, limits, power/posture/pilot guards, rollback and restart have
coverage. Facing remains available through detached `btech.unit.state` inspection.

`btech.unit.speed` and `btech.unit.heading` now stage the native control confirmation
for cockpit occupants, retaining boolean success results. Callback rollback
restores both requested motion and staged output. Controls remain numeric in Lua;
native stop/walk/run/back resolve to the same domain speed setter.


`btech.unit.scan(unit, pilot, target, options)` returns the same styled detailed
unit report as native `scan`. Optional A/I/W selections restrict report sections.
It requires a callback transaction and the assigned conscious pilot, but consumes
no dice or simulation state. Detached unit state exposes computer-derived
`sensor_ranges`. Observer disclosure and distance exemptions retain the shared
visibility, LOS and hardware guards.

Successful native/Lua scans now stage the same target warning through
`scan_battle_unit_action`. Observer scans and shutdown targets stay silent.
Warnings use the target's own contact visibility, and callback rollback discards
staged output. The lower-level Rust `scan_battle_unit` query still publishes nothing.


`btech.unit.scan_hex(unit, pilot, x, y, options)` implements native `scan x y`,
with optional report sections in Lua. It selects the first acquired visible
occupant in saved map order and shares unit-scan reports and warnings. Empty and
unacquired hexes return the same reply without changing contact state or dice.


`btech.unit.scan_building(unit, pilot, x, y)` implements native `scan x y B`.
It returns `{text, experience_messages}` and stages cockpit/channel output through
the shared action. Hidden-building perception dice, eligible XP, and output roll
back together if the callback fails. Explicit coordinates retain hardware range
limits for observers; invisible and missing buildings share the same reply.


`btech.unit.scan_terrain(unit, pilot, x, y)` implements native `scan x y H` and
returns `{building, mines}` while staging both phases' output. Building and mine
perception share the XP interval. Failed mine detection is pilot-only; recognized
mines reach the cockpit. Callback rollback restores both phases' dice, XP and
output. `btech.unit.scan_hex` continues to inspect unit occupants.


`btech.unit.scan_selected(unit, pilot, options)` implements scans of the saved
target and returns `{kind, report}` for unit, building or full-hex dispatch.
It preserves selection/countdown state and shares the target-specific output and
rollback path. Selected-coordinate observer range exemptions retain visibility
and hardware checks; explicit coordinate scans keep their existing range policy.


`btech.unit.report(unit, pilot, target)` returns the same silent brief summary as
native `report target`. It shares detailed scan admission except for the direct
report's distance exemption. The query changes no state or output and provides
no armor or weapon details. Native coordinate/default forms resolve their target
before calling that same query.


`btech.unit.view_center(unit, pilot, kind, arguments)` exposes shared tactical/
long-range centering without rendering. It returns `{map, center, maximum_range}`
for the own unit, a visible contact, or bearing/distance projection. Projected
centers may be outside the map; callers must apply viewport clipping and display
visibility. No state or output changes, and no terrain data is disclosed.


`btech.unit.viewport(unit, pilot, kind, arguments, dimensions)` resolves the shared
display center and clips requested dimensions into an in-bounds rectangle. Optional
dimensions use the standard defaults and limits. The detached result reports cell
counts, not inclusive end coordinates. It reveals no terrain/occupants and changes
no state; saved player preferences and rendering remain separate integration work.


`btech.unit.lrsmap(unit, pilot, mode, arguments)` matches native long-range T/E/M
displays and returns `{viewport, text}`. Rendering is read-only and silent, with
contact filtering and dark-map visibility applied before text output. Requested
centers share the same range/observer policy as native commands. Finer LOS display modes remain unfinished.


`btech.unit.lrsmap` now accepts C/`colored_elevation`. T/M honor the pilot's ANSI
flag; E stays plain; C shows numeric zeroes with terrain colors. Returned text uses
trusted styles with escaped cell glyphs and resets before row labels. Native and
Lua requests share palette selection and identical styled output.


`btech.unit.lrsmap` accepts L/H/S or `visible_terrain`, `visible_elevation` and
`visible_units` to apply the existing hex visibility query on ordinary maps.
Evaluated obscured cells render as blue question marks. Native/Lua text is identical;
full terrain/elevation mask tracing and directional terrain sensor parity remain open.


Visibility-filtered `btech.unit.lrsmap` output now uses live fire, inferno and
searchlight terrain illumination through the shared hex-visibility query. Native
and Lua views share that behavior without acquiring contacts or advancing state.


Tactical display coverage: native `tactical` and Lua
`btech.unit.tactical(unit, pilot, arguments?)` share the pure Rust
`battle_tactical_map` renderer. Standard, L visibility and U underlying-terrain
modes provide terrain/elevation hexes, acquired contact IDs, own marker, palette
colors, clipping and shared display-center admission. Rendering is read-only;
native/Lua, restart, contact privacy and odd-column layout checks cover this path.
Navigation overlays, player preferences and fine
terrain/elevation LOS masks remain open.

Tactical C/T cliff overlays now share the native/Lua renderer, with three/two-level
thresholds, signed water/ice depths, viewport-bounded edges, own-marker-only
output and dark-map rejection. Regression checks include restart and unchanged
simulation/outbox state. Navigation overlays remain open.


Tactical B now renders strict landing suitability in native/Lua output, with
base grass/road, six equal-height on-map neighbors and saved circular landing
exclusions. Team exemptions apply per circle; type-9 persistence preserves the
records across restart. The own marker remains, other contacts are omitted,
and dark maps reject the overlay. Tests cover terrain, radius boundaries,
exemptions, persistence and read-only native/Lua parity. Aircraft landing physics,
permissive landing configuration and native/Lua exclusion authoring remain open.


Tactical M now shares native/Lua rendering with first-field ordering, trigger
suppression and live unblocked terrain visibility. It shows only `<>`, retains
contact IDs and moves elevations above the marker space. Rendering consumes no
dice or perception checks and publishes nothing. Tests cover clipped placement,
visibility/dark maps, persistence and unchanged state. Navigation displays and
fine terrain/elevation LOS masks remain open.


`findcenter` and `btech.unit.findcenter(unit, pilot)` now share the current-hex
navigation report, using continuous position, horizontal range and clockwise
bearing. The readout needs a conscious assigned pilot and running unit but no
scanner hardware. Native/Lua parity, restart and unchanged-state checks cover it.
The combined `navigate` display is described below.


Native/Lua `navigate` now shares a radius-two local hex map, a continuous-position
compass plot and source position, terrain, speed and heading readouts. It preserves
off-map surroundings and permits an own-hex view with failed scanner hardware.
Regression coverage includes both parities, single-hex maps, visibility, remote
centers, persistence and read-only native/Lua parity. Fine terrain LOS and aircraft
navigation remain open.


Saved player dimensions now drive native/Lua tactical and LRS displays and Lua
viewport defaults. `btech.player.view_dimensions(player, dimensions?)` queries or
replaces validated sizes inside trusted callbacks. Existing configuration columns
are preserved on save; callback rollback, restart, invalid state and display parity
are tested. Navigation remains fixed-size. Contact-filter policy remains open.


Native `mapdisplay [width height lrs-height | reset]` now provides self-service
access to saved sizing, using the same setter as Lua player preferences. It only
addresses the invoking player and works outside a cockpit. Query, mutation,
validation, switch rejection, reset, Lua agreement and restart are covered.
Contact-filter preferences remain open.


Saved unit-contact categories now apply through `contacts +` and explicit Lua
`btech.unit.contacts(unit, preferences)` tables. Player preferences can be queried
or replaced through `btech.player.contact_preferences`. Selected targets may bypass
category filters, never acquisition/visibility. Shutdown and wreck filters remain
independent. Native/Lua, restart, rollback and sizing independence are tested.
Building contacts, per-call option strings and native category editing remain open.


Per-call unit contact options d/s/e/a/t and persistent ! exclusion mode now share
native parsing with Lua `btech.player.contact_options`. Parsing returns ordered
unknown-character diagnostics and never edits saved preferences. Native/Lua,
selection, visibility, restart and grammar-boundary checks cover this path.
Building option b uses the identification-lock path described below.


Building contacts now run silent identify_building locks through native `contacts b`
and Lua `unit.building_contacts`. Hidden denied structures and all invisible
structures are omitted; ordinary denied structures retain restricted status.
State and notices roll back if a lock fails. Tests cover callback identities,
concealment, visibility, native/Lua parity and restart. Brief formatting/sorting
and finer height-aware sensor traces remain open.


Saved building inclusion is now typed and persisted with contact categories.
Lua player.contact_preferences accepts buildings="include"/"exclude"/"follow_brief";
native contacts + resolves it, while explicit b remains independent. All modes,
restart, invalid input/database state and callback rollback are tested. Follow-brief
resolves the saved unit contact mode described below.


Unit-owned brief C0..3/A0..6 settings now have native and Lua query/edit paths,
validated persistence and atomic edit notices. C1 (default) controls implicit
building inclusion; A6 disables routine notices and A2/A3/A5 restrict them to
enemies, preserving weapon-lock warnings. Acquisition is unchanged. Tests cover
modes, option composition, Lua parity, rollback, invalid state and restart.
Exact contact layouts/sorting and automatic-notice text/color variants remain open.


Native contact ordering now follows the unit brief mode. C1/C2/C3 retain up to
250 mixed rows and sort descending by range plus 10,000 for destroyed units or
20,000 for structures. Candidates enter the bounded list in saved map membership
order, then entrance order. Equal keys retain that encounter order. C0 keeps map
membership order followed by entrances. Detached Lua unit queries remain ascending
by range; this presentation policy never changes acquisition or consumes dice.
Tests cover far/near units, a nearby wreck ahead of a distant unit, structure
priority, verbose ordering, Lua-query independence and restart. Exact row layouts,
headers/footers, status columns and automatic-notice formatting remain open.


Acquired contact reports now include a five-character `status`, shared by native
S: output and Lua unit.contacts. Columns cover carried club; destruction/lamp/light;
jump/prone/standing transition; shutdown/startup/heat/inferno; and homing beacon or
electronic warfare. Each column uses priority rather than concatenating conditions.
Narc/homing uses n for friendly and N for hostile contacts; electronic indicators
prefer ECCM P, ECM E, protection p, then disturbance e. Visibility is checked before
building the report, and blocking terrain blanks all condition columns. Queries
never mutate unit state or consume dice. Tests cover native/Lua agreement,
power/posture/heat/inferno priority, beacon team case, protection versus disturbance,
visibility and restart. Towing, swarming, hull-down, aircraft control and vehicle
fire indicators await those domain systems; exact row layout is still open.


Contact views now expose `sensors.primary` and `sensors.secondary`, and native rows
prefix P/S for the eligible roles. These are live eligibility results for an
already acquired target, not the stored observation snapshot or a new acquisition
roll. Identical modes share one evaluation and report both roles consistently.
The target is omitted when neither active mode is eligible. Tests cover primary
only, secondary only, both, neither, unacquired exclusion, unchanged saved state,
native/Lua parity and restart. Weapon-arc prefixes and exact row layout remain open.


Unit and building contact reports now include a typed `weapon_arc` (front/right/
rear/left), rendered as */r/v/l. The shared biped classifier uses whole heading
degrees, rounded compass bearing and the torso's 59-degree offset, favoring right
for a merged torso pose. Coincident points use the established 180-degree display
bearing. Arm flips do not change this general indicator, and individual mounts
retain independent firing checks. Contacts compute the arc only after acquired
visibility; building lock revalidation recomputes entrance geometry. Tests cover
boundary rounding, wrapped bearings, both arm poses, torso variants, native/Lua
agreement, query purity and restart. Vehicle turret arcs await vehicle support;
exact row layout and automatic-notice formatting remain open.


Automatic contact notices now honor long A0/A2 and short A1/A3/A4/A5 formats.
They include the battlefield label and observer-relative Forward/Right Arm/Left
Arm/Rear arc. Hostile acquisition uses red and loss uses yellow in A0..A3;
A4/A5 omit styling. A6 and observer units suppress routine notices while retaining
weapon-lock loss warnings. Friendly/enemy filtering remains independent of message
length. Names are made plain and escaped before styled output. Blocking terrain
uses "something" with an uppercase label; unavailable geometry suppresses only
the routine notice. Tests cover all modes, affiliation, acquisition/loss wording,
color, escaping and preserved lock warnings. Last-identified-name snapshots across
LOS loss and additional autocontact admission policies remain open.


Each saved contact now records whether its latest observation identified the unit
through clear terrain. Scanner transitions capture that flag before removing a
lost contact. Acquisition/loss notices use the captured flag for the display name,
label case and affiliation, so a previously unidentified signal remains
"something" rather than acquiring a name during notification. This stores the
identification fact, not a historical copy of a mutable name. Live sensor roles
use a separate BattleContactSensors report type and never rewrite stored history.
Tests cover persistence, identified and unidentified loss, removal, one-time loss
events and the existing native notice modes. Other autocontact admission policies
and exact contact-list layouts remain open.


Live contact views now expose current `identified` separately from acquisition.
A seismic contact retained through blocking terrain shows "something", blank
status columns, and no friendly classification. Category filters and tactical
markers use that same disclosed affiliation. The raw target handle and kinematic
sensor report remain available. An end-to-end ridge test covers native output,
Lua, category filtering, query purity and configured restart. It also verifies
that the runtime-only seismic stopped-target setting does not block database
saves; like skill policy, it is applied from configuration on startup rather than
stored as world identity data.


Routine acquisition/loss notices now suppress shutdown and starting targets by
default. `mechprefs AutoconShutdown ON|OFF` (or toggle without a value) changes the
unit-owned inclusion preference; Lua unit.autocon_shutdown(unit, pilot, enabled)
uses the same guarded setter, and unit inspection exposes autocon_shutdown.
The setting survives restart, is independent of player contact-list categories,
and participates in callback rollback. Acquisition and weapon-lock loss warnings
are unaffected. Tests cover default behavior, starting/running targets, native/Lua
edits, authority, rollback and persistence. Additional scenario/runtime notice
level overrides remain deferred with their owning systems.


C1/C2/C3 now render compact biped unit rows from a detached shared `short_text`
field. Rows contain P/S sensor flags, torso arc, battlefield label, B movement
class, a twelve-character plain name, x/y/z, one-decimal range/speed, whole-degree
bearing/heading and five status columns. Contact views also expose label,
coordinate and elevation for custom Lua displays. Friendly labels are lowercase;
unidentified contacts retain generic names and uppercase labels. Tests fix the
column spacing, bounded name behavior, mode routing, ordering, native/Lua parity
and restart. Building row formatting, list headers/footers, color, lateral heading
and the C0 verbose layout remain open.


Building contact rows now use P/S terrain roles, the torso arc, a 23-character
plain name, x/y/z, range/bearing, current/maximum CF and two status characters.
The detached short_text is finalized after identification-lock evaluation and
revalidation. Lua building_contacts and native output share that row. Rust
battle_hex_sensor_visibility exposes the same two sensor roles used by hex_visible;
identical modes share an evaluation. Tests cover exact columns, one/both/neither
terrain sensor roles, native/Lua parity, restart and existing lock/concealment
behavior. Row colors, list framing and the verbose unit layout remain open.


Compact unit rows now highlight selected targets in bold red, taking precedence
over affiliation. Other nonfriendly contacts use bold yellow; identified allies
retain default styling. Denied building-identification rows use bold yellow.
All data-derived row text is escaped before trusted styling, including battlefield
label brackets. Native contacts returns a styled document for capability-aware
rendering. Detached short_text stays plain; Rust styled_short_text(selected) and
building styled_text share the native formatting path. Tests cover selection,
affiliation, building locks, escaping, plain-text equivalence and styled dispatch.
List framing and the C0 verbose layout remain open.

`btech.map.wrapping(map, enabled)` shares the saved policy used by native
`@btech map-wrapping <map>=<on|off>`. It requires a callback transaction and returns
true on success; errors roll back with the enclosing callback. `btech.map.inspect`
returns the current boolean `wrapping` field. Native changes require Wizard access
and control of the target map, like other map administration commands.

Native `pickup <label|#dbref>` / `dropoff` and Lua
`btech.unit.pickup(carrier, pilot, target)` / `btech.unit.dropoff(carrier, pilot)`
share one host action, including prior-tow release, target shutdown, terrain
consequences, notices and rollback. Lua calls require the normal mutation context.

`@btech unit-towable <unit>=on|off` and trusted
`btech.unit.towable(unit, enabled?)` use the same per-unit permission. Lua omission
queries it. `btech.unit.state(unit)` exposes `towable`, `towing`, `towed_by`, and
`free_fall` through one shared projection for Mechs, ground vehicles and VTOLs.
