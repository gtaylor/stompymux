# Special-object dispatch and help contract

The reference registry has five object types: MECH, DEBUG, MAP, AUTOPILOT and
TURRET. The missing registry/help behavior is broader than adding global command
aliases. Its command candidates, authority and help sections belong to the
selected special object. The current Rust native command registry does not yet
implement that complete contract.

## Dispatch

`special/registry.c::btech_command_try_execute` checks the actor, the actor's
current location, then the actor's carried objects in contents order. A candidate
must be registered and must not have the Zombie flag. A matching command handles
the input even when access is denied; it does not fall through to a later object.
The command's first space-delimited word is matched case-insensitively through
the per-type table. Arguments skip ordinary spaces. Mech commands additionally
apply the signed class-mask gate: zero permits all, positive masks include named
classes, negative masks exclude them.

Restriction is indicated by a leading `@` in the help description, independently
of the command name. The rejection is `Sorry, that command is restricted!`.
God, Wizard or the special type's nonzero required power can bypass this gate.
All five current type definitions use POWER_NONE, so that last power path does
not grant ordinary users access to these restricted commands. This gate is
separate from the selected handler's own cockpit/control checks.

The HELP fallback has a case-sensitive uppercase check before the candidate
lookup lowercases the command buffer. Command-buffer mutation can also affect
later candidates. Before implementing HELP dispatch, verify its complete entry
path and supported casing with an executable fixture; do not assume this local
fallback accepts every casing simply because ordinary table lookup does.

## Help

`special/registry_help.c::btech_special_object_help` derives categories from
handler-less catalogue rows, applying the same authority and Mech class gates.
The command descriptions and order come from those catalogues. Help is not
alphabetically sorted and must not maintain a separate command inventory.

- Empty arguments list command names in four columns, grouped by visible
  category. A single section uses the type's command-listing title.
- Multiple sections advertise `HELP SUBTOPIC`; a single section advertises
  `HELP ALL`.
- `ALL` is case-insensitive but rejected for multiple sections with
  `ALL not available for objects with subcategories.`
- A named category requires a full case-insensitive match. Unknown categories
  report `Subcategory not found.`; single-section types instead explain that
  their detailed help is only available through `HELP ALL`.
- No authorized commands produces
  `There are no commands you are authorized to use here.`
- Detailed help strips a description's restriction marker, retains the full
  command syntax, and wraps descriptions with three-space indentation through
  the reference menu layout. Section headers use green styling and surrounding
  menu rules. The source's requested detailed column width is 37.

MAP currently has no category separators. Its public `STORES` entry means an
ordinary user can have a nonempty map command listing even when administrative
map commands are hidden. MECH has category separators and therefore does not
use an unrestricted all-categories HELP ALL page.

## Registration lifecycle

`commands/btech.c::do_btech` resolves a controlled target before registration or
unregistration. Registration requires a live Thing and a recognized type;
registering the same type again succeeds without reallocating it. Changing type
requires explicit unregistration first. The API checks object validity, then
control, then the requested type. Missing type input lists all five type names.

Unregistration is idempotent and also forgets the object's BattleTech
configuration. A registered object's disposal invokes its type-specific lifecycle.
Therefore a correct Rust implementation must route unregister through native
map/unit/turret cleanup and persistence rather than just delete a registration
string. DEBUG and AUTOPILOT registration also need their actual usable domain
state; a registration label alone would not close their behavior gaps.

## Implementation and acceptance still required

Use typed per-type command metadata shared by admission and help. Connect it to
the existing native handlers and transactional world services, avoiding a second
set of combat or map operations. Cover candidate ordering, denied-command
short-circuiting, Zombie exclusion, class masks, exact help category behavior,
registration idempotence, type conflicts, teardown, database restart and rollback.
The general MUX help system must remain available when no special object handles
the command. This document characterizes the missing contract; it does not claim
that special-object dispatch or help is implemented.

## Prepared catalogue data

`src/btech/special_commands.json` now retains the observable syntax, descriptions,
restriction markers, signed class masks and category order from all five reference
catalogues: 194 MECH entries, 9 DEBUG, 26 MAP, 7 AUTOPILOT and 25 TURRET. The ten
MECH category rows are explicit; null sentinels and C handler bindings are absent.
Descriptions store their restriction marker separately so rendering and admission
can share it. The data preserves oddities such as the unrestricted DEBUG SETWBV
entry and MAP's sole unrestricted STORES entry.

Extraction consumed every catalogue row and checked the terminators. Structural
checks cover the five types, field shapes, nonempty text, absence of implementation
bindings, category counts and selected restriction/class-mask cases. The asset is
consumed by `special_commands.rs`, whose typed queries share ordered metadata,
case-insensitive command matching, space-delimited arguments, signed class gates
and help visibility. Dispatch lookup retains restricted matches so the caller
can report denial without falling through; category rows cannot execute.
Missing Mech records and unknown classes retain separate behavior. All 252
library tests pass, including catalogue coverage and all signed masks from
-255 through 255 against the eight reference class bits. Ordinary-command
dispatch and registration lifecycle integration remain unfinished.

## Shared help renderer

`BattleSpecialType::help` now renders catalogue-derived menus without world
mutation. It applies the shared class and privilege filters, preserves category
order, supplies four-column listings and handles named categories and ALL with
the reference error messages. Detailed syntax uses a colored initial command
word; descriptions use three-space indentation and the reference wrapping
boundaries. Single-section titles, 70-column category centering, 78-column rules
and 19-column listing cells reuse the menu formatting used by weapon reports.

The library suite passes all 254 tests in
`target/audit-special-help-lib-final.log`; all five weapon-report integration
tests pass in `target/audit-special-help-weapons.log`. Tests cover public MAP
listing and detail, Mech class/privilege categories, exact category matching,
empty authorization and fixed display widths.

## Live HELP routing

`special_dispatch.rs` now selects existing registrations on the actor, location,
then inventory in saved linked-list order. It uses the existing relationship-order
projection for unsaved membership changes, rather than a second inventory list.
Zombie candidates are skipped. The saved unit identity supplies class filtering,
including imported identities whose simulation is not implemented. DEBUG and
AUTOPILOT registrations can therefore show help without claiming usable domain
operations or new registration commands.

An isolated probe executes the original reference `handled_command_sub`,
`okay_hcode` and `btech_command_try_execute` functions with stubbed registry and
notification services. Its 16 cases verify uppercase HELP, lowercase/mixed-case
fallthrough, actor/location priority and deliberately reversed inventory order
with Zombie exclusion (`target/reference-help-probe.log`). This is a dispatcher
probe, not a complete reference server session. The main reference command path
was also read to establish whitespace compression before BattleTech dispatch.

Rust native integration covers cockpit classes, public imported object help,
uppercase/general-help separation, configured whitespace behavior, persisted
inventory order, Zombie fallthrough, state preservation and restart. Registration
commands, lifecycle teardown, ordinary command gates and handler selection remain
open requirements of the full registry work.

Verification passes 254 library tests, nine special-help/maintenance integration
tests, and 24 general-command/help tests in `target/audit-live-special-help-lib.log`,
`target/audit-live-special-help-final.log` and
`target/audit-special-help-dispatch-regression.log`.

## Ordinary-command restriction gate

`special_dispatch::admit` now shares candidate traversal and class selection
between HELP and ordinary command checks. The first permitted class match ends
the search. A restricted entry returns the exact reference denial unless the
executor is GOD/Wizard; a queued cause cannot grant privilege. A public or
privileged MECH/AUTOPILOT/TURRET match continues to the existing native dispatcher. It does not yet
redirect that handler to the selected special object or replace the global
handler's own admission rules. In particular, catalogue-public entries such as
AUTOPILOT EVENTSTATS still need their proper handler adapter.

Tests cover case-insensitive restricted matching, names with and without `@`,
denial before an identically named exit, unchanged state, restart, public-first
short-circuiting, Zombie exclusion and queued executor/cause separation. All 62
integration tests pass across `target/audit-special-admission.log`,
`target/audit-special-admission-final.log` and
`target/audit-special-admission-operators.log` (the final admission log repeats
the two admission tests with the added exit-collision scenario).

## Selected map command execution

MAP matches now retain their selected object through invocation. Nineteen map
adapters use one explicit target resolver while sharing the existing mutation,
notification, validation and rollback services. Selection releases its world
borrow before the handler runs. Native command lookup uses the matched catalogue
name, so unrelated global aliases cannot redirect the selected command.

Two overloaded names need distinct existing handlers: VIEW invokes map viewing;
STORES invokes the actor-location manifest, rather than cockpit cargo stores.
The reference `mech_manifest` reads `game_object_location(player)` even when the
selected object is a carried map. Stock correction handlers likewise keep their
actor-location target. No actor movement or duplicated map rules are used.

All 26 MAP entries have handler bindings. Integration compares 21 map operations
through carried-map and direct-location selection, checks that the other map
and actor location remain unchanged, and covers restart, queued map-actor
priority, a same-named exit and stock-target exceptions. All 360 tests pass in
`target/audit-map-dispatch-regression.log` and
`target/audit-map-dispatch-selected-final.log` (255 library and 105 integration).
Other special types still need handler adapters. Global native fallback remains
available and needs type-scoped cleanup with those adapters; registration and
teardown are also unfinished. This is selected-map routing acceptance, not a
claim of complete special-object dispatch parity.

## Selected DEBUG command execution

All nine DEBUG entries retain catalogue selection before global aliases or exits.
Existing statistics, save, forms and experience handlers are reused. SETVRT and
SETWBV have a shared command adapter for the reference argument errors, bounds
and exact very-long equipment names. The shared part-name index rejects short
names, wildcard patterns and non-weapons. The reference VERY_LONG matcher uses
an exact hash lookup; its LONG matcher alone performs wildcard matching.

SETWBV is public in the reference DEBUG catalogue. This adapter uses the same
typed settings mutation as the Wizard-only operator and Lua APIs, without
borrowing another actor's authority or relaxing those APIs. Runtime overrides
still reset on reload. SETVRT retains Wizard admission and staged audit logging.

DEBUG SHUTDOWN requires an explicit map number even when the actor is in a
cockpit. Missing map registrations produce no reply; existing maps use the
shared atomic shutdown and map-clearing service. Extra arguments are ignored.

The integration test `tests/btech_debug_dispatch.rs` covers public access,
restricted controls, exact error messages, manufacturer names, exit precedence,
restart, global/Lua authority isolation and shutdown argument handling. A library
check verifies bindings for all nine commands. Registration lifecycle and other
special-type adapters remain open, as does complete reference output acceptance
for the reused DEBUG services.

Final routing regression: 272 tests passed (256 library and 16 integration) in
`target/audit-debug-dispatch-final.log`; formatting and diff checks passed.

DEBUG character controls now use shared full character-name lookup. Canonical
names win before short aliases in catalogue order across values, advantages,
attributes and skills. SETXPLEVEL distinguishes invalid arguments, invalid
integers, negative thresholds, unknown charvalues and non-skills in reference
order. XPTOP distinguishes empty arguments, unknown names and known non-skills.
Threshold mutation and logging still use the shared operator transaction.
The final run passes 265 tests (257 library, eight integration) in
`target/audit-debug-character-final.log`, including exact replies and unchanged
state after rejection, silent alias-based zero thresholds, operator rollback,
and the existing native/Lua XP report tests.

## DEBUG registration lifecycle

A controlled live Thing can now be registered with `@btech/register <thing>=DEBUG`.
Use `@btech/info <thing>` or `@btech <thing>` to inspect it, and
`@btech/unregister <thing>` to remove its DEBUG registration. The `info`, `register`
and `unregister` switches accept one-character abbreviations. Commands require
Wizard access and control of the matched object; Wizard command authority does
not grant control of another protected Wizard object.

Registering the same type is idempotent. A conflicting type requires explicit
unregistration. Removing an absent registration succeeds without creating state.
Objects and their containment are preserved. DEBUG owns only its registration,
so its persistence writer changes only the registration row inside the existing
world-save transaction. Other types retain their existing domain-specific save
and teardown guards; unsupported creation/removal reports that limitation.
Map, unit, autopilot and turret lifecycle work remains open.

Lifecycle verification passes 283 tests (257 library and 26 integration) in
`target/audit-debug-registration-final.log`. New integration checks native
creation, reload, use, idempotent teardown, final reload, named inspection,
short switches, live-Thing validation and protected-object control.

## MAP initialization

`@btech/register <thing>=MAP` now uses the shared map constructor with the
reference allocation defaults: a 21x11 grassland grid at elevation zero, name
`Default Map`, raw gravity/temperature/flags zero, light two, visibility 30,
maximum visibility 60, cloud base 200 and building regeneration one. No second
terrain or environmental implementation is introduced. Same-type registration
returns success without resetting a loaded or edited map.

The registered object can immediately supply selected VIEW and LOADMAP commands.
The new integration scenario checks every initial hex, environmental defaults,
read-only viewing, idempotent registration, save/reload, loading differently sized
terrain and a second save/reload, preserving object containment throughout.
All 269 tests pass (257 library and twelve integration) in
`target/audit-map-registration-final.log`. MAP teardown remains unfinished.

## MAP teardown

MAP unregistration now shares the existing bulk shutdown, casualty and placement
services, then removes the map identity and all its owned terrain, events and
map-object records. The underlying object, its physical contents and loose
inventory remain. Cleanup notices go to GOD as in the reference lifecycle;
admission still checks the actual executor's Wizard access and object control.
Going maps can be unregistered. Failure restores the outer world/effects
checkpoint, and persistence removal participates in the existing database save.

Authored configurations pointing at the map are cleared. Other maps' explicit
entrance and exit objects retain their destination object IDs: the destination
can cease to be a MAP without ceasing to exist. Loader and world checks preserve
that distinction, while missing or garbage objects remain invalid. Building
repair loading skips inactive destinations, and building damage returns no
building impact for them. Re-registering the object as MAP makes its retained
markers usable with the new interior again. Full object deletion continues to
use its separate, broader dependency cleanup.

Final teardown verification passes 294 tests (257 library and 37 integration)
in `target/audit-map-teardown-final.log`. The new tests cover all supported
chassis, inventory and containment, retained markers, Going maps, reactivation,
output and database rollback, and firing at an inactive building destination.

## TURRET lifecycle and batched role changes

TURRET registration now allocates the reference unattached defaults: parent and
gunner zero, target and x/y minus one, z/arcs/lock mode zero. Attached station
creation shares the same typed defaults while setting its chosen parent and
unassigned gunner. Four TIC words are now explicit station-owned saved state;
creation writes all four required rows, inspection preserves their values, and
teardown removes them with station-owned lock/correction events. The parent
unit's existing firing-group behavior remains shared.

Unregistration preserves the underlying Thing and its occupants. It does not
clear another station's events merely because a raw parent/target field points
at the retired station object. Going stations can relinquish their role.
Validation and persistence retire old roles before constructing replacements,
so explicit unregister/register changes among DEBUG, MAP and TURRET can share one
save. Nine transitions are tested, including same-type TURRET recreation resetting
nonzero TIC words. Opaque imported MAP-record replacement remains outside this
matrix and requires further acceptance; other type adapters and MECH/AUTOPILOT
lifecycle also remain open.

Final lifecycle regression passed 281 tests (257 library and 24 integration) in
`target/audit-turret-lifecycle-final.log`, with the three lifecycle tests repeated
successfully in `target/audit-turret-lifecycle-selected.log`. Formatting and diff
checks pass; the reference tree remains unchanged.


## Selected turret fields and initialization

Nine of the 25 turret commands now consume their selected actor/location/carried
candidate before general exit lookup: `@SETTURRET`, `@VIEWTURRET`, `INITIALIZE`,
`DEINITIALIZE`, and the five deliberately inert TIC commands. Field controls
retain Wizard admission; lifecycle and reserved TIC entries remain public.
The selected-object resolver is shared with MAP adapters.

Station admission accepts the actor itself, its location or a directly carried
station. Explicit station APIs retain this accessibility boundary. Initialization
checks the previous connected gunner's location against the actor's location,
not the station identity. A connected gunner elsewhere permits takeover.
Repeated initialization uses the reference joystick text. These operations keep
physical containment, parent pilot/equipment and independent targeting unchanged.
The remaining 16 turret commands still need selected-object adapters.
