# BattleTech implementation

For current scope, validation and follow-ups, see
[the first implementation summary](btech-first-implementation.md).
The detailed entries below include historical milestones.

The current integration scope excludes autopilot and game repair systems, naval
units, aerospace units and dropships, infantry, and battle armor. Mechs, ground
vehicles and VTOLs remain in scope.

Loading a unit into another unit, unloading it, and container cargo handling are
also deferred. External towing remains in scope. Existing transport-loss behavior
is retained; container transport is not a prerequisite for completing this phase.

The runtime uses native Rust domain code. It does not compile, link, or invoke
the C btech implementation. LuaJIT and SQLite remain dependencies.

Rules that do not depend on anatomy are shared across unit types. `launch_roll`
owns attack/propellant dice, loader failures, Streak locking and glancing; firing
mode fallback and gatling supply rules are shared as well. `weapon_groups` sizes
packets for Mech, vehicle and terrain targets. `unjam` owns one feed-clearing
workflow, with `unjam_unit` limited to accessing the different owned state. Unit
adapters retain cockpit authority, inventory writes and anatomy-specific damage;
new unit support should extend these shared rules rather than copy them.

`blast_damage` owns live occupant selection, altitude bounds, blast facing, pilot
protection, material packets and heat responses for both mines and artillery.
Callers provide each cell's origin and height limits, select source-specific
messages and retain their own terrain effects. Targets are inspected immediately
before their impact, so preceding damage can still change later eligibility.

## Available now

- Bounded map-file decoding into typed terrain and elevation, with shared immutable
  tile storage. Dimensions are width followed by height; environment lines use
  `flags: gravity temperature`.
- Biped BattleMech template decoding into named sections, armor, internal structure,
  critical slots, equipment names, modes, brand metadata, and unit-level fields.
  Critical ranges use one-based file positions and zero-based Rust positions.
- Wizard `@btech` map creation/reload and asset and saved-object inspection, with ordinary command access
  policies, aliases, switches, and background execution handling.
- Callback-scoped Lua map creation/reload, tile queries, and asset and saved-object inspection through `require('btech')`.
- Copy-on-write terrain and saved map/unit identity projections in world checkpoints. Database
  cleaning removes their registrations and clears surviving units' map references
  consistently with relational cleanup.

**Supported Mechs, ground vehicles and VTOLs have live movement and combat; complete gameplay parity remains unfinished.** Parsing equipment metadata is
not equipment validation. A parsed template must not be activated until its
equipment, modes, features and movement rules are implemented. Saved registrations
are likewise not active simulated objects.

## Inspection interfaces

Wizard commands:

```text
@btech status
@btech template JR7-D
@btech loadout JR7-D
@btech unit-create #44=JR7-D
@btech unit-place #44=#43,0,0
@btech unit-remove #44=#2
@btech mapfile test.map
@btech inspect #42
@btech range #44,#45
@btech map-create #43=test.map
@btech map-reload #43=test.map
```

The two asset commands resolve names under `database.mech_database` and
`database.map_database`, relative to the configured game directory. Parent-path
traversal, absolute names, symlink escapes, malformed UTF-8 and oversized files
are rejected. Template files are bounded at 1 MiB; map files at 2,100,000 bytes,
with at most 1,000 tiles on each axis. No operation writes an asset.

Trusted Lua uses:

```lua
local b = require('btech')
local definition = b.template.inspect('JR7-D')
local loadout = b.template.loadout('JR7-D')
local asset = b.map.inspect_file('test.map')
local unit = b.unit.inspect(42)
local saved_map = b.map.inspect(43)
b.map.create(44, 'test.map')
b.map.reload(43, 'test.map')
local hex = b.map.hex(43, 0, 0) -- zero-based coordinates
```

All operations require a game callback and reject checking mode. Results
are detached tables; editing them does not mutate the world. `btech.errors`
contains the existing structured error-code tree. Template read/parse failures
raise `btech.template.invalid`; missing saved records and map-file errors raise
`btech.operation.failed`. Bad argument types retain normal Lua argument errors.

Template inspection returns `name`, `reference`, `tons`, `max_speed`, `jump_speed`,
`heat_sinks`, `attributes`, and `sections`. Section keys are `LeftArm`, `RightArm`,
`LeftTorso`, `RightTorso`, `CenterTorso`, `LeftLeg`, `RightLeg`, and `Head`.
Each section includes `armor`, `internal`, `rear`, `criticals`, and optional
`configuration`. Critical keys are zero-based positions, with `equipment`, `data`,
`modes`, and optional `brand`. This is parsed source metadata, not a mutable unit.

Map-file inspection returns dimensions, gravity, temperature, and flags, without
materializing a potentially million-tile Lua table. Saved map inspection adds its
stored name and `terrain_ready`. `map.hex` returns `terrain` (a snake_case name)
and `elevation` (0–9), and rejects undecoded maps or out-of-bounds coordinates.
Saved unit inspection returns `name`, `template`, `class_code`,
`movement_code`, `tons`, and optional numeric `map` dbref.

## Equipment resolution

`@btech loadout <template>` and `btech.template.loadout(name)` resolve equipment
into distinct weapon mounts, independent ammunition bins, and individual system
criticals. Locations contain a named section and zero-based slot; multi-slot
weapons have a list of locations. Rear-mounted weapons and optional brand metadata
are retained. Returned Lua values are detached and require callback scope.

The conventional catalog covers Inner Sphere small, medium and large lasers, PPCs,
flamers, machine guns, SRM-2/4/6, LRM-5/10/15/20 and AC/2/5/10/20, plus
conventional actuators, engines, gyros, cockpit, sensors, life support,
heat sinks and jump jets. Weapon facts include heat, damage, missile count, ranges,
critical size, ammunition capacity and variable recycle time. Ammunition counts
are complete salvos; adjacent ammunition criticals remain separate bins.

Unknown equipment, unsupported modes, invalid ammunition quantities, and incomplete
or inconsistent multi-slot weapon runs fail with section/slot diagnostics.
Multi-slot mounts require contiguous runs. AC/20, LB/20-X and Heavy Gauss mounts may use
explicit split links into an adjacent section, as described below. Resolution
does not validate the entire chassis, feature flags,
construction weight, manufacturer effects or simulation support. It is the typed
input for unit construction, not permission to activate combat simulation.

## Unit construction

`@btech unit-create #object=JR7-D` constructs a conventional biped on an unused
live thing controlled by the wizard. Trusted callbacks use
`btech.unit.create(dbref, template_name)`. Both publish a checked candidate and
commit through the world transaction. Failed callbacks or saves cannot persist
partial registration or construction state.

`btech.unit.state(dbref)` returns detached `definition`, `sections` and
`ammunition` tables. The definition is the owned parsed template; section keys
match template section names and contain current armor, internals and rear armor.
Ammunition entries count complete salvos in resolved bin order. New units start
undamaged with the asset's ammunition quantities. `unit.inspect` provides the
same identity projection for constructed units and deferred saved units.

Construction currently accepts 20–100 ton conventional bipeds using the initial
equipment catalog, standard section slot capacities, conventional core systems,
and `FlipArms`. Unknown chassis fields, configurations and specials are rejected.
This is not yet a full construction-weight or chassis-legality verifier.
Running combat remains deferred.

Owned records live in the version-1 `btech_units` table as bounded serialized Rust
unit state, with ordinary `MECH` registration. It is the authoritative storage for
constructed units, not a whole-world JSON snapshot. Existing `btech_mechs` runtime
rows remain untouched and inspectable; a database cannot contain both kinds of
record for the same object. Fresh databases include the empty table; existing
databases install it in the first construction transaction. Restart does not
consult source templates. Invalid unit state or unsupported versions fail loading,
and object purge deletes the unit record in the enclosing cleanup transaction.

## Pilot entry and exit

Players use ordinary `enter <unit>` and `leave`, preserving their existing access
policies, callbacks and movement messages. Inside a constructed unit, `pilot`
claims its cockpit after the unit's `USE` policy passes. `unpilot` releases the
cockpit without leaving the unit. Neither command accepts arguments or switches.
Other occupants remain passengers. The unit does not need to be on a battlefield
to accept a pilot, and claiming a cockpit does not start its engine.

Only one player may pilot a unit, and the pilot must remain physically inside it.
Departure through ordinary movement or Lua teleport clears the assignment before
arrival callbacks. Failed movement restores it. Pilot and unit destruction are
reconciled in the same cleanup transaction. Pilot identity survives restart.

Trusted callbacks can use `btech.unit.pilot(unit,player)` and
`btech.unit.release(unit,player)`. These enforce cockpit availability and physical
presence but delegate access policy to the calling script. `unit.state` includes
an optional numeric `pilot`; wizard inspection also reports it. Callback guards
and world checkpoints apply to all crew mutations.

## Geometry and range

Shared Rust geometry uses hex-height units with positive y pointing south. Even
columns are offset half a hex south; odd columns are not. Bearings run clockwise
from north. The six discrete neighbors and shortest hex-step distance use cube
coordinates internally, while projections and physical range use continuous
coordinates. Graph steps and Euclidean range are distinct quantities.

`@btech range #unit,#unit` and trusted Lua `unit.range(first,second)` measure placed
units on the same decoded battlefield. They return horizontal and spatial range,
clockwise bearing (absent for coincident centers), and `hex_distance`. Spatial range
uses signed ground heights, including water depth, at five elevation levels per
hex height. Invalid identities, unplaced units and different maps are rejected.

These are geometry queries, not sensor contacts: they do not evaluate LOS or
weapon eligibility. Continuous positions are used once units move; range no longer snaps moving
units to hex centers. Continuous coordinates use f64;
reference anchor tests allow the source implementation's float rounding.

## Weapon readiness and expenditure

`weapon_readiness` reports intact mounting slots, total usable matching ammunition,
remaining recycle time and mechanical readiness. `spend_battle_weapon` requires
the assigned, present pilot and a running, ready unit. Conventional ammunition is
selected from the weapon's section first, then in section/slot order; empty or
destroyed bins are skipped. One shot consumes one salvo, including for missile
launchers. Energy weapons do not consume bins. Rejected requests alter neither
ammunition, timers nor heat. The same mutation adds weapon heat, whether the
attack hits or misses. The result identifies the weapon, spent bin and heat
already added; callers must not add that heat again.

Recycle timers are owned unit state keyed by zero-based weapon index and exposed
in Lua `unit.state.weapon_recycle`. They advance one second per committed server
tick while running, pause during shutdown, and resume from saved remaining time.
Stationary units still receive recycle ticks. Completion notifications share the
same commit boundary; failed saves restore the timer and discard notices.
Destroyed units clear timers, and broken weapons cannot become ready. Invalid
saved timer indices or durations fail validation.

The firing action still needs targeting/LOS, hit probability, overheat consequences,
crew/fall effects and command/Lua controls. This expenditure API must be composed
with those rules in one transaction; it is not itself a firing command. Ammo-bin
preferences, special firing modes, temporary weapon failures and limb recycle
remain pending.

Unit-target aim adds +1 when the shooter is in water below elevation zero. The
`attacker_water` report field exposes the term separately from movement. A target
standing in water does not cause this attacker penalty; coordinate-only terrain
attacks do not add it. This does not enable fully submerged weapon firing.

Air-target modifiers use the same target calculation for all shooter types.
A VTOL with nonzero horizontal or vertical speed adds +1 to the target movement
term. LBX cluster rounds receive -3 against VTOLs, including landed aircraft,
instead of their ordinary -1. Stinger rounds receive -3 against flying VTOLs,
-1 during orbital descent (including jump-jet compensation after cocoon loss),
and no extra bonus against ordinary jumping Mechs. Orbital ground vehicles are
valid Stinger targets. These terms appear in previews and actual firing.

## Conventional weapon salvos

`BattleWeapon::missile_hits` implements unmodified 2d6 cluster tables for the
supported SRM-2/4/6 and LRM-5/10/15/20. `damage_groups` gives each SRM a separate
two-point impact and combines LRMs into groups of at most five one-point
missiles. Conventional lasers, PPCs and autocannons produce one damage group without a cluster
roll. Invalid rolls and incompatible weapon/roll combinations are rejected.

`battletech.divrotordamage` scales external VTOL rotor damage when positive.
Damage is divided before armor material adjustments, with a one-point minimum
for positive hits. Other sections and direct internal damage are unaffected.
The default, zero, disables scaling.

Configured firing uses `battletech.moddamagewithrange` for energy weapons: +1
at actual range <= 1 hex, -1 beyond medium, or integer half damage beyond long.
This follows weapon degradation, precedes glancing rounding, and retains at
least one damage point. The default is disabled. Mech, vehicle and terrain
attacks share the calculation; underwater range profiles remain pending.

`resolve_battle_salvo` resolves a successful weapon hit against a world candidate.
Each group receives its own location roll and conditional critical/head-graze
rolls using the target's current state, then runs the whole-hit resolver. Groups
stop when the target is destroyed. All rolls, groups and nested damage publish
together; normal persistence commits them atomically and restart resumes the
same dice stream. The report carries each group's hit, damage and impact effects.

This is not yet a firing command: targeting/LOS, hit probability, expenditure,
heat, recycle timing and the pending crew/fall effects still need integration.
AMS, Artemis, Streak, special ammunition, cluster modifiers, terrain absorption
and partial cover also remain pending. Unsupported weapon templates remain gated.

## Whole-hit material and critical resolution

`resolve_battle_impact` accepts an already selected hit and damage amount. It
resolves armor, through-armor criticals, internal criticals, section loss and
transfers against a private unit candidate, then validates and publishes the
candidate once. Conditional dice and all nested equipment changes belong to that
same world checkpoint. A zero-damage hit consumes no dice and makes no changes.

Conventional critical rolls yield one critical on 8–9, two on 10–11, and three
on 12. An internal 12 instead severs an arm, leg or head and cancels its overflow,
unless through-armor criticals already supplied the critical effects. Ammunition
critical damage executes immediately before the interrupted hit resumes. It
bypasses armor throughout its transfer path; the bin is cleared before recursion
so it cannot explode twice. Each cascade has a bounded work allowance and fails
without publishing any candidate state if resolution cannot complete.

The result includes ordered material phases, selected equipment losses and
pending head/explosion injuries, crew stun and section-loss effects. Weapon
handlers still need to apply those casualty/fall effects, notifications, firing
permissions, ammunition consumption, heat and recycle timing before committing.
This resolver is not yet a playable firing command. Specialized armor/structure,
CASE, inferno ammunition and other unsupported equipment remain out of scope for
the conventional resolver and must be added before those templates are enabled.

## Damage and mobility

`BattleUnit::mobility` derives a damage-adjusted speed limit and piloting modifier
from saved section and critical losses. A missing biped leg limits speed to
10.75 kph and adds five to piloting checks; two missing legs prevent movement.
A damaged hip halves speed and adds two, overriding other actuator damage in
that leg. Other lost leg actuators each subtract 10.75 kph and add one. The
reference's running-speed calculations and left-leg/right-leg processing order
are retained. Two lost hips or two gyro hits prevent powered ground movement.
A damaged conventional gyro contributes a piloting modifier of three.

Damage immediately clamps current and desired speed to the new bounds. The tick,
numeric speed requests and `speed run|walk|back` all use the derived maximum;
immobile units reject heading changes. Lua inspection includes `mobility`, and
restart validation rejects saved motion outside the damage-adjusted limits.
These limits precede terrain, heat, cargo and advantage modifiers. Fall damage,
prone/standing state and actual piloting checks remain pending; the piloting
modifier is ready for those rules rather than a claim that they already execute.

## Equipment losses

Units persist destroyed critical slots in `lost_criticals`, also available through
Lua unit inspection. Random selection uses only occupied, surviving slots in the
selected section; an exhausted section consumes no dice and does not transfer the
critical. A weapon becomes nonfunctional after one mounting slot is destroyed,
while its other slots remain eligible for later critical hits. Section destruction
marks its installed slots lost along with its armor and ammunition.

`destroy_battle_critical` marks a selected slot inside a world checkpoint and
returns the affected weapon, system or ammunition bin. Ammunition results capture
the removed rounds and conventional full-salvo explosion damage before the bin
is cleared. Three engine hits or cockpit destruction kill the unit, cancel power
and stop motion; core structure does not need to be zero for these kills.
Destruction releases the pilot assignment while leaving occupants inside the
unit object. This follows `unit/mech_lifecycle.c`'s destruction/power-down contract.
Claiming that cockpit again does not permit startup or firing from the wreck.
`weapon_intact` and `system_hits` provide derived equipment availability. Invalid
and repeated requests do not alter state. Persisted losses must identify installed
slots, and destroyed ammunition bins must be empty.

The whole-hit resolver applies conventional ammunition explosions. The enclosing
combat action must still apply heat,
sensor penalties, falls, pilot casualties and notifications before
committing. These slot primitives are not complete critical-hit gameplay commands.
Enhanced weapon-critical damage and specialized equipment remain pending.

## Material damage phases

`apply_damage_phase` operates on a unit inside the caller's world checkpoint.
The armor phase consumes front armor, or rear armor for rear torso hits, and
returns overflow. Combat must resolve critical effects before passing remaining
damage to the internal phase. Internal overflow transfers to the next section's
armor using `damage_transfer`; destroyed locations absorb nothing.

Losing a section clears its protection and stored ammunition. Losing a side torso
also removes its attached arm. Head or center torso destruction stops motion,
cancels startup/running power and prevents startup. The MUX object remains present
for wreck handling. Lua `unit.state.destroyed` and wizard inspection report this
condition. Pilot references remain available for subsequent injury/death handling.
All material changes share normal unit persistence and world rollback.

These are damage-pipeline primitives, not complete firing or damage commands.
Critical-driven explosions/severing, falls, pilot
injury/death, heat effects, notifications, salvage and specialized protection
remain pending. A combat handler must resolve those effects and commit the whole
action together, never independently commit its material phases. Leg loss adjusts mobility; the tactical resolver also applies its forced fall on supported land or water.

## Biped hit-location rules

`BattleHitArc` classifies a target-relative bearing using the three `hit_arcs`
layouts. Modes 0 and 2 share the biped 180-degree front and 60-degree rear arcs;
mode 1 uses four 90-degree arcs. Exact front/rear boundaries belong to those arcs.
These are continuous bearings; parity with the reference's integer bearing
quantization near boundaries still needs paired scenarios.

`BattleHitTable` resolves supplied weapon (2d6), punch (d6), and kick (d6) rolls.
`BattleHitRules` adds through-armor critical eligibility and the Exile head-hit
variant. FASA critical rules make a weapon location roll of two a candidate.
Standard rules use front armor percentage, including for rear hits: below 60%
there is a 1-in-12 chance, at 100% a 1-in-71 chance, and at 60–99% no chance.
A section with no original front armor is always eligible. Head-hit rerolls use
the punch table; Exile mode 1 also reports a stun for non-head grazes, while modes
above 1 only reroll. Rear armor applies only to torso hits from the rear.

Resolution returns a section and pending critical/stun effects. It does not
apply damage, destroy components, schedule stun recovery, emit messages or
establish that a shot can be fired. Combat handlers must apply those effects and
commit their dice together. `BattleSection::damage_transfer` supplies conventional
biped overflow destinations for the damage system. Critical-proof equipment,
quad and other unit classes, aimed shots, partial cover and their rule variants
remain pending. These rule primitives are not yet exposed as gameplay commands.

## Transactional dice

Each constructed unit owns an independent, system-seeded ChaCha8 dice stream in
its saved unit record. `BattleDice` carries an explicit `chacha8-v1` algorithm tag;
unknown tags fail loading. Bounded rejection sampling produces fair d6 results,
and ordinary 2d6 checks sum two independent dice. The generator is provided by
`rand_chacha`; game code owns the dice mapping and persistence contract.

`roll_unit_dice` consumes a bounded set of dice within a world transaction. Rule
handlers must validate their action before rolling, use the acting unit's stream,
and commit outcomes and updated dice state together. A failed action restores the
whole checkpoint. Saving and restarting resume the same stream. There is no
player or Lua command to consume, reseed or inspect future rolls; Lua unit state
uses an explicit public projection. Unit records require their dice state rather
than silently inventing a replacement stream when state is missing.

This is infrastructure for combat and piloting rules; firing, skill checks, roll
statistics and game notifications are still pending. Independent unit streams
preserve dice probabilities, not the reference engine's global seeded sequence.

## Ground movement

A running unit's assigned pilot can set `heading <degrees>` and
`speed <kph|stop|walk|run|back>`. Headings wrap around the compass. Walking and
reverse speed limits are two-thirds of maximum running speed. Desired controls
are distinct from actual motion: the tick turns gradually and accelerates or
brakes by one-twentieth of maximum speed per step. Server turning uses
`battletech.fasaturn` and `battletech.slowdown`.

Ground displacement follows the reference's MOVE_MOD and coordinate scale:
current speed / 645 hex heights per tick at the standard map movement rate.
A positive saved `move_mod` scales displacement by its percentage; zero or
negative values select the standard rate. Every crossed hex is checked using
segment intersections, so accelerated movement cannot skip an obstacle.
Rough ground, snow and light forest halve target speed; mountains and heavy
forest divide it by three. Sub-hex position, current/desired heading and
current/desired speed persist in `unit.state.motion`. Lua controls are
`unit.heading(unit,pilot,degrees)` and `unit.speed(unit,pilot,kph)`.

The initial movement slice supports level grassland, road, forests, rough ground,
mountains and snow. Map edges, elevation changes, water/ice, bridges, fire, smoke,
buildings and walls stop before entry. These are explicit pending rules, not substitutes for
falls, flooding, bridge/building behavior or map links. Shutdown-related falls
are covered by the engine lifecycle below. Jumping, heat, damage modifiers,
boosters, pilot advantages and towing remain pending. A blocked segment retains
the position at the beginning of its tick; partial movement up to the obstacle
and crossing-related gameplay effects remain pending.

Continuous-to-hex conversion selects the nearest center, with south/east ownership
of exact shared boundaries and rectangular clipping at the left edge. This is a
stable f64 convention; exact boundary ties are not yet paired against the C float
implementation. Coordinate anchors and ordinary neighbor geometry are verified.

## Engine startup and the simulation tick

An assigned pilot on a decoded battlefield can issue `startup`. The conventional
biped startup takes 30 committed one-second steps, with six occupant messages five
steps apart. Wizards may use `startup override` for the final five-step sequence.
Both modes reject excess heat strictly above 30 before changing any state. A
rejection preserves the assigned pilot, does not schedule a countdown, and allows
a later retry after cooling. The limit is checked when startup is requested.
`shutdown` aborts startup or stops the engine and releases the pilot assignment.
Stopping a running ground unit above positive speed 10.75 applies the shared fall
resolver (Mechs also resolve stacking); equality and reverse motion do not trigger
this tumble. Jumping Mechs and elevated mobile vehicles enter their existing saved
descent paths instead. Native and Lua shutdown publish fall damage, character
injury and evacuation in one transaction, rolling back state and output together
if any consequence fails. Aborting startup does not apply a shutdown fall.
Taking the cockpit again is required before another startup. Administrative unit
placement requires the engine to be off. Map destruction also powers down survivors.

`unit.state` includes `power`, with `state` equal to `off`, `starting` or `running`.
Starting state includes `remaining` seconds. Trusted Lua uses
`unit.start(unit,player,fast)` (optional `fast`, default false) and
`unit.stop(unit,player)`. Script callers authorize the fast override themselves.
These operations guard callback scope and atomically roll back state and messages,
including failures caught by an enclosing Lua `pcall`.

A dedicated server interval drives one step per second through the ordinary world
commit boundary. Pending steps are persisted in the unit record; failed commits
restore their state and discard output. Retry happens on the next tick. Idle units
do not trigger writes. Downtime and delayed ticks do not cause a burst of catch-up
steps; startup resumes from its last committed countdown after restart. Startup, ground motion, thermal accounting and weapon recycle use the tick; remaining combat
transitions remain pending.

## Battlefield placement

`@btech unit-place #unit=#map,x,y` assigns zero-based ground hex coordinates and
moves the unit object into the map container. The wizard must control both objects.
`@btech unit-remove #unit=#destination` clears the coordinates and moves the unit
into an ordinary room or thing. Trusted callbacks use `unit.place(id,map,x,y)`
and `unit.remove(id,destination)` with the same transactional domain operations.
These are administrative placement operations, not simulated movement.

`unit.state` includes an optional `position` with `map`, `x` and `y`; identity
inspection also exposes its map. Placement preserves armor and ammunition, permits
multiple units in a hex, and rejects ambiguous terrain, invalid coordinates,
unavailable objects and containment cycles. Placed units block terrain reload.
Remove them before an ordinary world move; writes that disagree with a unit's
battlefield location are rejected. Removing the last unit and reloading terrain
can commit in one transaction. Destroying a map clears surviving units' battlefield
coordinates as ordinary object cleanup relocates their containers.

## Map creation and explicit reload

`map-create` registers an existing room or thing as a map. `map-reload` replaces
its terrain and environmental settings from the named asset. The wizard must
control the target. Lua `map.create` and `map.reload` are trusted callback
operations and return `true` on success. Both participate in callback rollback
and the enclosing database transaction; successful output follows commit.

Reload requires the same dimensions and no units on the map. Maps with saved map
objects or nonzero mine/hangar bitfields cannot yet be reloaded. These restrictions
protect state whose gameplay behavior is still deferred. Successful reload clears
the derived line-of-sight cache. Assets themselves are never modified, and asset
changes do not automatically replace persisted terrain.

## Terrain persistence

The base storage remains schema 32 with btech schema 8. A version-1 per-map
terrain dictionary lives in `btech_map_terrain` and `btech_map_terrain_codes`.
Fresh databases include these empty tables; existing databases add them atomically
with their first explicit map creation or reload. Ordinary startup and unrelated
writes do not install them.

The reference implementation assigns terrain-code bytes dynamically but does not
persist the dictionary. Such existing maps load as ambiguous metadata: raw bytes
are preserved, tile queries fail, and an explicit source-asset reload is required
to establish terrain. This reload replaces terrain; it cannot recover arbitrary
saved terrain edits. The source asset cannot prove the original code allocation.

Once a dictionary exists, every saved grid cell must decode. Unsupported versions,
missing tables, cells or codes, and invalid entries fail loading instead of
falling back to an asset. Reload preserves existing code assignments and appends
new combinations. Both grid and dictionary updates preserve unrelated columns;
map destruction removes its dictionary. Deferred unit state remains protected
against unsupported edits. No C runtime or bridge is involved.

## Verification

`tests/btech.rs` exercises copied JR7-D, AS7-D and map fixtures, malformed asset
rejection, command authority, detached Lua results, callback/checking restrictions,
unknown-data preservation, atomic save rejection, and map destruction with a
surviving unit. Unit tests exercise coordinate boundaries, depth, rectangular
maps, input limits, critical ranges and path confinement.

`tests/btech_motion_core.rs` verifies acceleration/displacement, fractional restart,
turning modes, terrain speed, invalid controls, rollback and edge stops. The TCP
power scenario also checks live server motion and braking before shutdown.

`tests/btech_geometry.rs` verifies reference center anchors, signed elevation/depth,
range adapters and cross-map rejection. Geometry unit tests check both parities,
compass projections, neighbor symmetry and invalid numerical inputs.

`tests/btech_power.rs` verifies all six startup stages, override access, shutdown,
countdown restart, invalid saved state, callback rollback, and a TCP server
countdown that recovers from a forced database failure without early output.

`tests/btech_crew.rs` exercises two units through enter, claim, restart and leave,
including a non-wizard player, USE policy denial/mutation, occupied cockpits,
callback rollback and pilot/unit destruction.

`tests/btech_placement.rs` covers independent placement, restart, coordinate and
containment rejection, callback rollback, occupancy and map destruction.

The weapon scenarios in `tests/btech_motion_core.rs` cover independent countdowns,
invalid expenditure, ammo exhaustion, shutdown/restart and a stationary live-server
timer that recovers from forced save failures.

`tests/btech_salvo.rs` verifies complete cluster distributions, group boundaries,
independent locations, repeatable salvos and save-failure/restart behavior.

`tests/btech_impact.rs` verifies repeatable whole-hit cascades, save failure/restart,
ammunition transfer through internal structure and limb severing without overflow.

`tests/btech_mobility.rs` and the damaged-unit motion scenario verify actuator
precedence, hit-order independence, missing legs/gyros, live speed clamping,
command/Lua controls and restart.

`tests/btech_critical.rs` verifies multi-slot weapon failure, selection depletion,
ammunition accounting, engine/cockpit kills, Lua inspection and restart.

`tests/btech_damage.rs` verifies front/rear protection, phase accounting, overflow,
side-torso/arm loss, ammunition removal, core shutdown, Lua/wizard inspection,
restart and failed-save rollback.

`tests/btech_hit.rs` verifies all 36 weapon-roll combinations per arc, physical
location constraints, transfer paths, arc boundaries, conditional critical dice
and head-hit reroll/stun variants.

`tests/btech_dice.rs` verifies seeded replay, restart continuation, save-failure
retries, invalid roll requests and exclusion of private dice state from Lua.

`tests/btech_units.rs` verifies construction defaults, unsupported-feature rejection,
independent units, durable definitions, callback/save rollback and purge cleanup.

`tests/btech_loadout.rs` verifies JR7-D/AS7-D weapon grouping, independent bins,
manufacturer/rear metadata, catalog values, rejection diagnostics and Lua guards.

`tests/btech_terrain.rs` covers dictionary round trips, explicit ambiguous-map
reload, malformed dictionaries, unknown-column preservation, independent maps,
callback/checking guards, purge cleanup, and server save-failure rollback.

These tests characterize source/data contracts; they do not claim paired C/Rust
combat execution. Tests never read or write the production `game/` database and
do not require the sibling C checkout.

The [coverage inventory](btech-coverage.md) tracks the remaining implementation.

## Conventional thermal accounting and environment

`BattleHeat` persists stored weapon heat and the most recent excess-heat sample.
Each committed second samples `max(0, stored + production - dissipation)`, then
changes stored heat by `(production - dissipation) / 30`, clamped at zero.
Dry-ground production includes walking/running and five points per engine
critical while running; cooling uses the surviving single heat sinks, including
implicit engine sinks. Shutdown stops engine production but permits cooling.
Stationary hot units still tick, and a failed save restores the complete thermal
state with the rest of the world. Running units and stable hot units also advance
their saved thermal-check clocks.
Lua exposes `unit.state.heat` without exposing the random stream.

Excess heat provides the conventional accuracy penalties at 8/13/17/24 and
indicator transitions at 14/19. The firing command applies these penalties.
The following periodic thermal pass applies tactical heat injury, ammunition
hazards and forced shutdown. Jumping, ice/bridge immersion, cutoff and special
sink types remain pending.

Heat also scales the movement target at excess-heat thresholds 5/10/15/20/25.
The damage-adjusted engine limit is converted to walking MP, rounded to nearest
with even ties, reduced by 1–5 MP, and converted back to running MP rounded up.
The resulting ratio scales the requested speed before terrain and turning
slowdown. A depleted movement allowance stops translation without reversing it.
The pilot's throttle setting, acceleration rate and damage-only mobility limits
remain intact, allowing gradual deceleration and recovery as the unit cools.
This calculation supports conventional bipeds; triple-strength myomer and speed
boosters remain pending.

`heat_rates(&world)` derives environmental effects from the occupied tile and
persisted map conditions. Fire adds five production points. Standing in depth-one
water adds cooling for at most four surviving leg sinks; deeper water adds six,
with total water cooling capped at twice the surviving sink count. Maps with the
special-conditions flag (mask value 2) adjust cooling by one point per started ten-degree
step below −30 or above 50; temperature metadata alone does not enable the rule.
Negative net dissipation in extreme heat is permitted. Off-map units use standard
conditions. Server activity detection uses these same rates, including stationary
running units that begin accumulating heat from an unfavorable environment.
Cold, shut-down units remain thermally dormant.

Water heat accounting applies to administrative placement in water; traversing
water still requires the pending movement rules. Ice/bridge falls,
ice immersion and bridge depth need further vertical-state handling, and are
not inferred from the current two-dimensional placement.

## Character health foundations

Rust loads the existing `btech_character_state` columns into `BattleCharacter`,
without creating missing profiles or interpreting the separate skills/advantages
table. `btech.character.state(player)` returns a detached inspection table during
a script callback. Trusted domain code can explicitly set a profile or call
`injure_battle_character`; player-facing stat editing is not exposed.

Cockpit injury adds two times build in bruising per hit. Bruising is capped at ten
times build; overflow becomes lethal damage. A fatal result leaves lethal damage
at one below capacity, matching the stored character-health convention, and must
be consumed by the enclosing combat action. Arithmetic uses wider intermediates;
operational builds are currently restricted to 1–25 so health fits the existing
byte-sized columns. Other stored profiles remain inspectable, but injury requests
reject unsupported builds or invalid live-health bounds before changing state.

Consciousness targets derive from bruising and build, with pain resistance as an
explicit input. `check_consciousness` uses ordinary 2d6 or the best two of 3d6
with toughness, returning the target, roll and outcome. Invalid health consumes
no dice. These helpers do not yet kill occupants, apply unit death or consume
pending impact effects. Those lifecycle
operations remain prerequisites for playable combat; fatal injury reports must
not be discarded by a future firing or heat handler.

Health writes update owned columns without replacing rows, preserving skills, XP
and independent columns. Ordinary world checkpoints cover character state; save
failure and purge use the same transaction boundaries as other world changes.
`tests/btech_character.rs` covers injury thresholds, lethal overflow, invalid
profiles, selective writes, failed-save retry, detached inspection and purge.

Consciousness uses a player-owned `BattleDice` stream so retries and restarts
reproduce recovery outcomes independently of cockpit assignment. The versioned
`btech_character_recovery` extension stores that stream, the remaining countdown,
and explicit pain-resistance/toughness settings. Reads do not install the table;
first writes install it atomically. Purging a player removes the recovery record.
Lua `character.state` exposes only `unconscious_remaining`, never future dice.

`check_character_consciousness` checks current health and sets a 30-second recovery
countdown after a failed initial roll. While already unconscious, further checks
update the supplied advantage settings without rolling or extending the timer.
Each recovery uses current character health; failure starts another 30 seconds.
Successful recovery sets the countdown to zero and notifies the player. These
steps run while stationary, off-map or without a unit. Failed commits restore the
countdown and dice together and discard notices. Cockpit assignment, startup,
shutdown, steering, speed and weapon expenditure reject unconscious pilots.
Releasing a cockpit does not erase the player's condition.

The remaining integration includes automatic advantage lookup, injury/heat
handlers invoking the check, non-character tactical injury rules, player movement
restrictions, death and unit-destruction handling. Advantage settings are supplied
by the enclosing injury action; recovery does not yet read the skills table.

Read-only recovery evidence: `unit/mech_events.c` retries failed recovery after
30 seconds (`UNCONSCIOUS_TIME` in `core/btconfig.h`) and clears unconsciousness on a
successful check. Initial injury while already unconscious does not roll again.
Reference unit-destruction cancellation remains part of pending casualty handling.

## Tactical cockpit injuries

`injure_battle_tactical_pilot` applies non-character cockpit injuries atomically
with player-owned consciousness state. It requires an assigned, present pilot and
rejects in-character units before mutation. Zero damage consumes no dice. Injuries
one through five use tactical consciousness targets 3, 5, 7, 10, 10, matching the
reference's four-injury lookup cap. Toughness is supplied by the enclosing action;
RPG profiles and pain resistance are not used for tactical checks. Recovery records
carry an explicit character/tactical mode, so tactical pilots need no character row.
Additional injury while unconscious updates the tactical count without rerolling
or extending recovery.

The sixth injury marks the unit destroyed through its persisted `pilot_injuries`,
stops motion and engine power, clears weapon recycle, releases the pilot and
cancels pending recovery. This is a tactical scenario casualty, not deletion of
the player's MUX object or mutation of RPG health. Restart cannot revive that unit.
Lua `unit.state.pilot_injuries` reports the count; the enclosing action publishes
the returned injury notice with its commit.

`resolve_battle_tactical_impact` composes conventional material damage, cockpit
injury, flooding and balance consequences with explicit `BattleFallRules`.
Head injury occurs before material damage; explosion injury follows its nested
damage cascade. Surviving assigned pilots receive one head injury or two explosion
injuries. All effects must succeed before publishing the candidate. Applied injuries
are removed from pending effects; unpiloted-target handling, section notices and
casualties after structural destruction remain explicit. The lower-level impact/salvo APIs
remain available for assembling the complete firing resolver.

Remaining casualty work includes in-character death, automatic advantage lookup,
pilot replacement/startup injury initialization, and applying these
operations throughout salvos, heat, falls and other injury sources.

## Cockpit stun

`stun_battle_unit` applies ten committed seconds of BattleMech cockpit stun.
Repeated hits reset the countdown to ten rather than adding durations. When the
unit is currently moving forward above walking speed, the commanded speed is
reduced to walking speed; actual speed changes through ordinary deceleration.
The throttle adjustment precedes material damage and critical mobility changes.
Reverse motion and slower forward motion retain their throttle settings. Stun
blocks weapon readiness/expenditure, but does not impose unconsciousness or an
additional blanket movement-control restriction.

Stun is owned unit state, exposed as `unit.state.stun_remaining`. Its countdown
runs even while stationary or shut down, survives restart, and rolls back with a
failed save. Only a running, surviving unit announces recovery. Destruction clears
stun along with weapon recycle. `resolve_battle_tactical_impact` consumes surviving
units' pending stun effects and returns occupant notices alongside applied pilot
injuries; callers publish these notices with the enclosing attack transaction.
The low-level impact resolver continues to return effects for other compositions.
Vehicle crew stun and physical-attack restrictions remain separate pending rules.

## Tactical salvo composition

`resolve_battle_tactical_salvo` uses the same cluster and hit-location traversal as
`resolve_battle_salvo`, applying tactical cockpit injuries, stun, flooding and
balance consequences before continuing to the next critical or group.
Each `BattleSalvoGroup` reports its material impact, applied pilot injuries and
staged occupant notices. The material-only resolver leaves these additional lists
empty and continues to expose unresolved crew effects in the impact report.

The complete salvo owns one world checkpoint. Internal impact/injury helpers reuse
that candidate, so a later failure discards earlier material damage, pilot state,
notices and both unit and recovery dice. Pilot loss stops subsequent groups just
like structural destruction; skipped groups consume no hit-location dice. Saved
state includes every completed group and resulting pilot condition.

This is still a successful-hit resolver. Target acquisition, line of sight, hit
probability, shooter expenditure and remaining casualty effects must be
composed by the firing action before exposing player-facing firing commands.

## Conventional aim modifiers

`battle_aim_modifiers` reads a placed shooter, target and weapon index and returns
an inspectable breakdown: supplied gunnery, spatial distance/range bracket,
attacker movement, target movement, heat, damaged sensors, mounting-section damage,
selected-target settling and the best eligible optical mode.
It is read-only, consumes no dice, and does not establish firing permission.
`subtotal()` sums only those contributions and returns no value beyond the
weapon's effective physical range or without a current acquired optical contact.

Conventional ranges use the reference's raw maximum cutoff and fractional minimum
penalty, then bracket distances with `floor(distance + 0.95)`. Extended range reaches
twice medium range when that exceeds long range, with a +8 extreme modifier.
Medium and long modifiers are +2/+4. Minimum-range rounding is characterized
separately, including the small interval just above nominal minimum range.

Ground attacker movement uses actual speed and the original template's walking
threshold. FASA turning adds one while standing/walking, but a running attacker
stays at +2; reverse movement follows the walking branch. Target movement uses
absolute speed with 2/4/6/9 MP bands and the optional extended-speed rule. Shutdown
or an unconscious assigned pilot contributes the immobile-target −4. Merely being
stationary or mechanically unable to walk does not imply that modifier.

Sensor damage contributes +2 for the first critical and 75 for loss of both sensors.
Arm shoulders override upper/lower-actuator penalties with +4; leg hips override
other leg actuator weapon penalties. These values are derived from surviving slots,
so damage ordering does not accumulate duplicate penalties.

This is not a complete attack target number. Firing arcs,
underwater fire restrictions, jumping, electronic systems, special equipment,
remaining rule variants still need composition.

## Biped weapon arcs and upper-body facing

`BattleFacing` persists torso orientation and flipped-arm state independently of
movement heading. `rotate_battle_torso` moves one step left/right (opposite turns
return through center), or centers directly; attempts beyond the limit fail without
mutation. `flip_battle_arms` toggles only chassis declaring `FlipArms`. Both require
a running unit and its present, conscious pilot. Lua unit inspection exposes the
saved `facing` record. Native `rottorso <left|right|center>` and `fliparms` commands stage occupant
notices through the command transaction. Lua mutation bindings remain pending.

`WeaponMount::bears_on` handles front, side and rear mounting arcs, 59-degree torso
offsets, flipped-arm side coverage and rear-mount precedence. Leg mounts use body
heading independently of torso orientation. Front edges include ±60 degrees; rear
edges exclude ±120 degrees. `battle_weapon_bears_on` queries placed units on the
same battlefield using current motion and pose; coincident points use a southward
bearing, matching the reference's zero-vector convention.

These geometric queries use the rewrite's continuous bearings and headings. Exact
reference integer-bearing quantization, prone restrictions, unavailable-arc rules,
and composing the result with LOS and the firing action remain pending. The arc
result alone does not indicate that a weapon is functioning or ready to fire.

## Terrain line of sight

`battle_unit_terrain_los` returns terrain observations between standing ground
BattleMechs on a shared battlefield. `ground_terrain_los` provides the same query
for explicit map hexes. Both are read-only and calculate from current terrain;
there is no contact cache to invalidate after placement or terrain reload.

Reports distinguish terrain blockage from intervening woods, target woods,
smoke, fire, mountains, water and partial cover. Standing eye height is ground
plus 1.5 levels. The source hex does not obscure its own observer, and target
woods are counted separately from intervening woods. Hills at or above the sight
height block the path. Underwater paths check the sea floor and water/air
boundary, and record attenuation for sensor selection. Counts retain the
reference limits of 15 woods and 7 water hexes.

This is terrain LOS, not final visibility: sensor range, lighting, sensor mode,
contact rolls and target acquisition still need composition. The query uses hex
centers and evenly advances sight height across the traced cells. Exact reference
hex-edge tie selection is still pending. Prone and airborne eye heights, intact ice surfaces and bridge deck endpoints
are supported. Full layered terrain positioning remains pending.

## Optical sensor eligibility

`battle_optical_contact` composes current terrain LOS and spatial range for visual
and light-amplification sensors on standing BattleMechs. `BattleSensorMode::evaluate`
exposes the rule calculation for a supplied LOS report. The result separates
eligibility, a sensor acquisition factor, and the lighting/woods/partial-cover aim
contribution. No query rolls dice or creates a contact.

Visual sensors reject three intervening woods points, smoke, fire and blocked
terrain. Light amplification rejects two woods points, any counted water and lit
targets. Underwater visual targets require fewer than six counted water hexes.
Target woods contribute to aim separately from intervening woods eligibility.
At night, illumination triples visual weather range and light amplification
doubles it; map maximum visibility and the sixty-hex optical limit still apply.
Visual aim loses its darkness penalty against lit targets. Light amplification
uses integer-halved darkness and integer-scaled woods penalties.

The explicit-input query accepts lighting, weather visibility, disabled status and
target illumination. The saved-map query below supplies lighting and weather from
persistent state. Installed sensor selection,
cloud boundaries, additional sensor modes, hidden/small/airborne targets, contact
rolls, pilot/team/arc weighting and primary/secondary state remain to be integrated.
The acquisition factor is one input to a later detection calculation, not the final
probability of acquiring a target. The aim contribution alone is not a firing
solution.


## Saved battlefield visibility

`set_battle_map_visibility` changes light (night/twilight/day) and weather visibility
(0–60 hexes), deriving a maximum range clamped to 24–60. The existing database
light, visibility and maximum-visibility columns are owned by the Rust map record;
loading preserves a stored ceiling even when it differs from the setter formula.
Decoded maps reject invalid conditions. Terrain reload preserves the saved settings.

Conditions can be changed and saved on occupied maps. Saving conditions updates
map metadata and clears obsolete saved LOS entries without rewriting terrain or
deferred map objects. `battle_map_optical_contact` uses those saved conditions and
honors the stored ceiling plus the optical hardware limit. Target illumination
and disabled-sensor status remain explicit pending their runtime integration.
Lua map inspection exposes light, visibility and maximum_visibility. Wizards can use `@btech map-conditions #map=<night|twilight|day>,<visibility>`
with control of the map. Trusted scripts use `btech.map.conditions(map, light, visibility)`;
callback failures roll back these changes with other world state.

## Optical acquisition rolls

`roll_battle_optical_detection` combines saved-map optical eligibility with a
standing-mech acquisition attempt, consuming only the observer's persisted dice.
The report includes detection, the strict threshold and the rolled value (absent
for ineligible targets and automatic acquisition below three hexes). The target's
dice and condition are unchanged. `BattleSensorReport::roll_detection` exposes
the same rule calculation with an explicit dice stream.

The attempt accounts for front/side/rear direction, hostile-target perception,
hidden hostile targets and secondary-sensor weighting, preserving integer division
order. Hidden hostile targets beyond five hexes are ineligible. At three hexes
and beyond, the roll is uniform from 1 through 10,000 and must be strictly below
the threshold. Failed validation and no-roll paths leave the stream unchanged.

Direction, perception, team/hidden status and primary/secondary role are currently
explicit inputs. This action returns an outcome; persistent contact ownership,
scanner cadence, sensor selection, contact notifications and in-character perception
XP still need composition. It is not exposed as a player reroll command or Lua
mutation. Restart tests verify that subsequent attempts resume the same dice stream.


## Paired optical scans

`scan_battle_optical_target` derives front/side/rear sensor direction from current
position, heading and torso pose, then attempts the requested primary and secondary
modes in order. A primary success suppresses the secondary attempt. Selecting the
same mode twice makes one full-weight attempt, including when that attempt fails.
Distinct secondary modes use the reference's half-weight acquisition threshold.
Arm flipping does not alter sensor direction; torso rotation uses 59-degree offsets.

Both optical eligibility reports are prepared before rolling, and observer dice
are published only after the scan succeeds. The returned report identifies the
successful sensor and retains only the attempts actually performed. Unavailable
and automatic close-range paths consume no dice. Tests compare paired scans with
explicit single attempts to verify order, weighting and unchanged target state.

This composes acquisition rules but does not store a contact or run periodic scans.
Sensor selection/availability, target light/team/hidden facts and perception are
still supplied by the caller. Current continuous-bearing boundary limitations also
apply to scanner direction.

## Durable optical selection

Units now own an active optical pair (initially visual/visual) and an optional
requested pair with a ten-second countdown. `select_battle_optical_sensors`
requires the running unit's present, conscious pilot. Selecting a new pair starts
or replaces the countdown; selecting the already-active pair leaves any pending
request alone. The active pair continues operating until completion. The countdown
survives restart and advances through the server's normal save boundary even while
stationary. Expiry while shut down discards the request without changing active modes.

Light amplification can be selected only at night or twilight. Changing map light
to day falls back to visual modes. Completion rechecks lighting, so a request made
at night cannot activate unavailable amplification after sunrise. This deliberately
validates at application time as well as request time. The reference applies pending
modes without that second check. Lua unit state exposes active and pending selections.

Players inspect with `sensor`/`sensor verbose` and request modes with `sensor V L`
(or supported names). Passengers can inspect; only the running unit’s conscious
pilot can change modes. Trusted Lua uses `btech.unit.sensors(unit, pilot, primary, secondary)`
with the same guards and callback rollback. Selection-aware periodic contact scans
and target-lock clearing/notifications remain to be connected. The completion notice currently
reports only the sensor change because target locks are not implemented.

## Persistent optical contacts

`update_battle_optical_contact` uses the observer's saved active sensor pair to
acquire, retain or lose one target. Acquired contacts are saved with the observer
as target object references and primary/secondary observation flags. Once known,
a target is retained without another acquisition roll while either current sensor
remains eligible. When neither can observe it, the contact is removed without a
roll. An optional observation-only mode updates known contacts without acquiring
unseen ones. Self-contact and invalid/cross-map targets are rejected.

The returned transition (`unseen`, `acquired`, `retained`, `lost`) lets a later
scanner loop publish notices once. Detection dice and contact ownership belong to
the same world candidate. Restart preserves ownership, so refreshing a known target
does not reroll it. Contacts are last observations, not a substitute for refreshing
visibility before firing. Administrative placement/removal clears incoming and
outgoing contacts; purge removes dangling unit/map references, and world validation
rejects self, empty, missing or cross-map records.

Periodic scanner scheduling, team/perception/illumination/availability inputs,
contact display and notifications, and target-lock loss are still pending. This
trusted domain action has no player-facing or Lua reroll interface.

## Saved sensor availability

The Rust map record now owns the existing `sensor_flags` column. Visual and
light-amplification sensors read bits zero and one; all other bits are retained.
`set_battle_map_optical_sensor` enables/disables one optical mode without changing
other sensor bits or occupied terrain. Lua map inspection exposes the saved bitfield.

Saved-map optical queries, paired scans and contact updates honor disabled modes
in addition to any explicit caller restriction. A disabled sensor cannot acquire
a close-range target; refreshing an existing contact loses it when neither mode
remains available. Selection itself is retained, so re-enabling a mode makes it
eligible without a sensor switch. These metadata changes use the normal map save
boundary and survive restart.

Automatic scanner scheduling still requires character perception and team/target
facts. The reference schedules possible-contact checks at one-second intervals;
player perception is loaded from character skills, not a universal constant.
The rewrite does not yet expose those skill targets, so this work does not enable
periodic acquisition with guessed player inputs.


## Character values and perception

Named skill/advantage rows now belong to Rust state as exact value names, byte
levels, unsigned 32-bit experience words and use timestamps. Writes update only
changed entries, preserving other names and unrelated columns. Character purge
removes the owned values as well as attributes and recovery state. Unknown value
names remain data; this does not claim support for their gameplay rules.

`BattleCharacter::skill_target` implements athletic, physical, mental and social
attribute pairings. Effective skill includes the stored experience word's high-byte
level bonus; its low 24 bits remain the experience balance. Reads do not award XP,
recompute earned levels or alter timestamps. `battle_perception_target` uses mental
attributes and the exact `Perception` value. Missing character data contributes
zero attributes/skill, yielding the reference target of 18; nonplayers are rejected.
Lua character inspection exposes detached values and the calculated perception target.

`set_battle_character_value` is a trusted domain setter requiring existing character
attributes. Player editing, the full value catalog, XP awards/training and scanner
startup caching remain pending. These formulas now provide actual saved perception
inputs for subsequent automatic scanner integration.


## Automatic tactical scanners

The one-second server tick now refreshes contacts for running out-of-character
BattleMechs with another supported target on their map. Observers are selected
before startup advances, so newly started units begin scanning on the next tick.
Targets are processed in stable object order. Known contacts are refreshed without
rerolls; acquired/lost transitions stage basic cockpit notices until the same save
that persists contacts and observer dice. Failed saves restore the entire tick.
Stationary units participate, and unchanged observations do not produce repeat notices.

Units own a `BattleSensorSignature` (team, hidden, illuminated), editable through the
trusted `set_battle_sensor_signature` domain operation. Defaults are team zero,
visible and unlit. Startup completion captures the actual player's perception target;
without a player pilot it uses the reference fallback of six. Later skill edits take
effect on the next startup. Lua unit inspection exposes the signature and cached target.

`optical_scanner_observers` and `refresh_optical_scanners` expose the domain scheduler
for deterministic tests. Refreshing a batch is atomic and duplicate observer IDs do
not grant extra acquisition attempts. Map sensor-disable flags and saved active
modes are applied automatically.

Automatic scans currently skip in-character observers (perception XP side effects
remain pending). Ice and bridge endpoints participate using current unit altitude.
Such contacts remain historical until a supported refresh or removal; they must not
be treated as fresh firing visibility. Contact notices currently use unit object
numbers. Full contact identifiers, display/filter preferences, shutdown-target notice
preferences, target locking, lights/hiding actions and complete terrain support are
still pending. This tick refreshes known contacts each second as well as attempting
new acquisition, rather than maintaining a separate movement-dirty LOS cache.


## Current contact display

`visible_battle_contacts` and Lua `btech.unit.contacts(unit)` return detached,
range-sorted views of acquired contacts that remain eligible under current active
modes, map conditions, terrain and sensor-disable flags. Invalid, removed, going or
unsupported targets are omitted. Display does not acquire targets, consume dice
or update contact ownership; a historical record alone cannot reveal live position.

Conscious occupants of a running unit use `contacts` or `contacts #unit`. The
current display includes object number, chassis name, friendly/hostile relation,
range/bearing, heading and speed. Literal output prevents names being interpreted
as formatting directives. Sensor-specific contact IDs, prefix filters, abbreviated
and detailed preferences, building contacts and remaining formatting parity are
still pending.

### Unit target selection and settling

`lock #unit` selects a current acquired optical contact; `lock -` clears selection.
The conscious assigned pilot must operate a running unit. Reselecting even the
same unit restarts an eight-committed-second countdown. Selection and countdown
live in native unit state and survive restart. `btech.unit.lock(unit, pilot,
target_or_nil)` uses the same guards; `btech.unit.state(unit).target_lock` is a
detached inspection with `target` and `remaining`. Commands reject switches.

Settling does not roll for acquisition. Read-only visibility checks leave selection
untouched, allowing the countdown to finish silently before a scanner refresh.
A contact refresh reporting loss clears selection and cancels its countdown. When the countdown expires with a
current visible contact, occupants receive a stable-lock notice, once and only
after the enclosing world commit succeeds. A zero countdown is historical
selection state, **not authorization to fire at an invisible target**. This
matches the selected-unit/timer distinction in `combat/mech_combat.c`,
`combat/mech_bth.c`, `unit/mech_events.c`, `sensors/mech_sensor.c` and
`core/btconfig.h` (eight seconds).

Shutdown, destruction, explicit clear, actual sensor switching, and administrative
placement/removal clear selection. Purging a selected unit clears incoming locks.
Daylight fallback also clears locks if it changes the active sensor pair. Saved
state rejects self/missing/cross-map targets, stopped observers, and countdowns
outside 0..8. An automatic scanner loss also stages a weapon-lock-lost notice.

This is the conventional optical unit-selection subset: hex/building targets,
map contact IDs, targeting computers, arc-override timing, sixth-sense warnings,
aimed sections and complete firing/to-hit orchestration remain pending. Native
selection notices currently use plain generic text. Lua mutation follows the
existing callback transaction and leaves notification policy to its caller.

Tests cover guard atomicity, restart mid-countdown, repeated selection, invisible
expiry, sensor/shutdown/placement cancellation, command/Lua access and callback
rollback, and a live server completion retried after a failed database write.


### Optical and selected-target aim contributions

Aim now uses fresh terrain/light/visibility/disable-flag evaluations for the active
sensor pair, gated by an already-acquired contact. No aim query acquires a target,
consumes dice or mutates saved observations. The primary contributes its optical
modifier; a distinct secondary contributes its modifier plus one. The lowest
eligible result wins, preferring primary on ties. Duplicate modes are evaluated
once. The selected mode and secondary surcharge are explicit in `optical`;
no eligible mode gives `None` and prevents a subtotal. Forest, darkness, target
illumination and partial-cover penalties come from the shared optical evaluator.
The reference applies shallow-water partial cover only with at least two LOS
steps; an adjacent shallow-water target has no cover modifier.

The conventional lock modifier is two without a selected unit, zero for a settled
selection matching the shot, and otherwise one if the **selected** target lies
in the torso's forward arc or two outside it. This intentionally follows the
selected target's bearing even when inspecting a shot at another target, as in
`combat/mech_bth.c`. `BattleAimRules.override_weapon_arcs` bypasses this penalty.
Settling and current eligibility remain separate: a settled selection does not
make a stale contact shootable. Sensor role selection follows
`sensors/mech_sensor.c::mech_sensor_to_hit_bonus`.

Tests cover no/pending/settled/different-target locks, torso-adjusted arcs, the
arc override, duplicate sensor modes, primary ties, secondary-only visibility,
illumination, target woods, shallow-water cover, stale/unacquired contacts and
restart stability. This is still an inspectable conventional subtotal, not a fire
command: weapon arc/readiness checks, posture, advanced
equipment, firing expenditure and damage effects must be composed by that action.


### Saved pilot gunnery

`battle_gunnery_target` reads the conventional biped skill from a player's saved
attributes and named values. With extended gunnery disabled all supported weapons
use `Gunnery-Battlemech`; with it enabled, conventional lasers and PPCs use `Gunnery-Laser`,
SRM/LRM launchers use `Gunnery-Missile`, and autocannons use `Gunnery-Ballistic`.
The target is 18 minus reflexes, intuition and the effective saved skill, including
its encoded earned-level bonus. Missing character data yields 18; missing skill
data uses zero skill with the existing attributes. Names retain exact catalog case.

`battle_unit_gunnery_target` resolves the currently assigned, present, connected
pilot or returns the reference default of six. Connection state is transient;
character values and experience persist. Unlike startup's cached perception,
gunnery is evaluated when requested. Separate gunner overrides, non-player crews,
other unit classes and XP awards remain pending. Skill inspection does not check
firing authority or readiness and does not update XP or `last_used`.

`battle_pilot_aim_modifiers` combines this lookup with the conventional aim
calculation. Explicit `battle_aim_modifiers` callers can still supply a target.
Gunnery is signed (`i16`), and subtotal arithmetic widens to `i32` so unusually
skilled characters and supplied numeric extremes cannot wrap. Neither API fires
weapons. Lua `btech.unit.gunnery(unit, zero_based_weapon_index)` uses configured
`extended_gunnery`, returns the signed target and requires callback context.

Reference evidence: `unit/mech_identity.c` selects skills and default gunnery;
`unit/mech_crew_state.c` and `unit/mech_lifecycle.c` define the normal gunner and
connected-pilot conditions; `character/character_value_catalog.c` supplies physical
skill categories; `character/btechstats.c` supplies target arithmetic.
Tests cover all five supported weapons, generic/specialized selection, missing
attributes/skills, XP bonus persistence, live skill changes, connection fallback,
Lua inspection and invalid indexes, and signed subtotal extremes.

### Direct tactical shot composition

`resolve_battle_shot` resolves one conventional shot as a private world candidate.
It requires a controlled running shooter, usable weapon, same-battlefield current
acquired contact, valid hit-arc mode and a live non-character target. Firing arcs
are checked unless the explicit arc override applies. Ice/bridge endpoints,
deep-water combat and submerged leg weapons require vertical combat rules and
are rejected before rolling or spending. Self-targets and stale contacts are
rejected. No rejected shot consumes ammunition, heat or either unit's dice.

Aim uses the connected pilot's saved gunnery, current optical mode, lock, motion,
heat and equipment. Its numeric target is calculated before shot heat is added.
Every accepted shot spends one salvo, adds heat, starts recycling and rolls the
shooter's 2d6 stream. A roll equal to the target hits; there is no unconditional
2-miss/12-hit exception for these conventional weapons. A visible target beyond
physical weapon range produces `target_number: None` and a miss, still spending
and rolling, as in `combat/mech_fire_resolution.c`. Misses consume no target dice.

Hits derive direction from the target's heading and the bearing toward the
shooter, and run the existing tactical salvo resolver. Partial cover uses one
upper-body (punch-table) d6 per group, preserving rear torso armor while bypassing
the ordinary weapon-table through-armor-critical/head-graze rolls. This follows
`combat/mech_hitloc_targeting.c`. Missile grouping, critical cascades and cockpit
injuries/stun occur in the same candidate. Any resolution error discards the whole
candidate. The returned `BattleShotReport` is marked `must_use` and contains the
roll, pre-shot aim, expenditure and optional damage report.

The cockpit `fire` command now uses this domain composition. Reported section
losses still need notices and any unsupported casualty handling; their supported ground/water
fall/posture consequences are applied by the shared tactical damage resolver.
Weapon malfunctions, XP awards, remaining water rules and the remaining
advanced equipment modes need integration. The caller owns database commit and
notification staging. Native and Lua firing expose the same supported tactical subset.
Tests compare threshold hits/misses with manual expenditure and damage, exercise
arc/visibility/readiness rejection, partial cover, missile salvos and out-of-range
expenditure, and force a target write failure to verify rollback of both units and
identical replay from the world checkpoint.

### Conventional piloting checks

`battle_unit_piloting_target` selects `Piloting-Biped` with extended piloting or
`Piloting-Battlemech` otherwise. Both use reflexes, intuition and the saved effective
skill, including encoded earned levels. It shares the gunnery lookup's connected,
present pilot check and fallback target six. Skill reads do not award XP.

`roll_battle_piloting` performs the conventional standing no-XP check used by fall
and movement resolution. The signed situational modifier is added to the current
skill and damage-derived mobility penalty (legs, actuators, hips and gyro). An
in-character unit without its pilot present also adds five. Subtotals use `i32`.
A running conscious unit rolls its saved 2d6 stream, succeeding on equality or
higher. Shutdown/startup and pilot unconsciousness fail without consuming dice,
even when numeric modifiers would otherwise guarantee success. Invalid/missing
units fail validation before rolling. Crew stun is separate from unconsciousness.

The returned `BattlePilotingCheck` includes skill, damage, cockpit, situational and absent
pilot contributions, total, optional roll and success. It is `must_use`: the caller
must apply the failed check's fall/movement effects and commit RNG with those effects.
This API does not notify, award XP, apply fall damage or implement blinding.
Already-prone units now succeed automatically without consuming dice, before the
shutdown/unconsciousness checks, matching the default reference check. Small
cockpits add a separate construction modifier of one to the target.

Reference: `unit/mech_identity.c` (skill names, target calculation, no-XP check),
`character/character_value_catalog.c` (physical skill category), `core/btconfig.h`
(default six). Tests verify generic/extended skills, hip/gyro modifiers, equality,
signed extremes, exact replay after restart, and no-dice automatic failure.


### Dry-ground falls and prone posture

`resolve_battle_fall(unit, levels, rules)` resolves a tactical ground or water fall in a
private world candidate. `levels` is the damage multiplier (one for an ordinary
fall), not an extra height increment. Before becoming prone the unit rolls to
avoid pilot injury with `levels` added to piloting; failure applies one tactical
pilot injury when a pilot is present. The fall then stops motion, resets torso
and flipped arms, and rolls a d6 for facing: 1 front, 2/3 right, 4 rear, 5/6 left,
with a corresponding clockwise heading change of 0/60/120/180/240/300 degrees.
Continuous fractional headings retain the rewrite's existing geometry convention.

Dry damage is `levels * (tons + 5) / 10`, using integer truncation. Special-map
rules scale it by gravity capped at 100%; higher gravity does not increase it.
Damage is applied in groups of five plus the remainder through the current hit
and tactical injury resolver, stopping on unit destruction. The report contains
the avoidance check, possible pilot injury, direction, total damage and groups;
no notification or database write escapes the enclosing transaction.

`BattlePosture` is serialized with the unit and exposed by Lua unit inspection.
Prone units have 0.5-level eyes instead of 1.5, do not receive standing partial
cover, and count as submerged in depth-one water. Prone attacker movement adds
two to aim; prone targets add -2 at range <=1 and +1 farther away, in addition to
immobility. Prone units cannot drive or twist their torso. Their upper-body pose
must be centered with arms unflipped and actual/desired speed zero. Restart
preserves posture and replayable fall dice. Prone water cooling uses immersion
rather than standing leg-sink cooling.

Reference evidence: `movement/mech_falls.c`, `unit/mech_lifecycle.c`,
`unit/mech_position_state.c`, `sensors/mech_los.c`, and
`combat/mech_bth_movement.c`. Tests cover fall direction, grouped damage, injury,
motion/facing reset, restart replay, prone LOS/aim and special-map gravity.

Water and high-water falls are supported, including section flooding in actual
water. Ice fracture and bridge surface selection are supported as described below. Prone non-leg weapons now use
the arm-support readiness rules described below. Heading changes are allowed
while prone and during stand timers. Tactical shots now apply ground/water falls
from criticals and leg loss. Collision and jump-failure triggers still need integration;
the cockpit command uses the shared shot API.


### Standing and failed-attempt recovery

`begin_battle_stand` checks the conscious assigned pilot, running/prone state,
remaining legs and gyro support, then resolves a no-XP piloting attempt in a
private candidate. It clears prone before rolling, so the already-prone automatic
success branch cannot bypass the check. Normal/careful attempts refuse an
unmodified target above twelve; `Anyway` overrides refusal. Careful attempts use
-2 when enabled. Failure resolves another one-level fall before setting recovery.

A successful attempt is upright immediately, with a `Rising` timer blocking
travel and adding the standing-up +2 attacker modifier. Failure remains prone
with a `Recovering` timer blocking another attempt. Base time is
`floor(30 / clamp(maximum_speed / 21.5, 1, 30))`. Careful success doubles this;
careful failure takes at least thirty seconds. Maximum speed is recomputed after
a failed fall, including newly damaged actuators. Timers persist and advance on
committed server seconds, including shutdown; destruction clears them. A new fall
cancels a pending successful rise. Administrative placement/removal cancels timers.

Native `stand`, `stand check`, `stand careful` and `stand anyway` use the same
contract. Inspection consumes no dice. The adapter uses configured extended
piloting, careful standing and hit rules plus the pilot's saved `Toughness`, staging
attempt and fall notices with the normal world commit. Lua `unit.stand(unit, pilot, mode)` returns the same attempt report and stages
the same notices; `mode` defaults to `normal` and also accepts `anyway` or
`careful`. Lua state exposes the read-only `stand_timer`. Timer validation rejects zero/oversized countdowns,
destroyed units and posture mismatches. Below-ice and in-character casualty handling remain outside the current
tactical stand path. Flooded legs count as unavailable support.

Reference: `movement/mech_move.c::mech_stand` and `mech_stand_time`,
`unit/mech_events.c::mech_stand_event` / `mech_standfail_event`. Tests cover immediate
upright state, movement lock, five/ten-second Jenner rises, thirty-second careful
failure, impossible target refusal/override, pure command inspection, restart and
server retry after a failed timer save.

### Recovery dice before first injury

Character setup and cockpit assignment now initialize a player-owned `Ready`
recovery record before injury is possible. This state has a saved private dice
stream, zero countdown, and no required health mode. Preparing it again preserves
all existing dice, advantages, health mode and recovery timing. The first injury
switches it to character or tactical mode and rolls the already-owned stream.
This closes the first-injury retry gap: generating fresh entropy inside a failed
injury transaction could otherwise produce a different roll on retry.

Server startup prepares missing streams for imported character records and
assigned pilots, and includes them in the ordinary startup commit before accepting
connections. It does not replace existing streams. Other Rust callers loading
raw game data use `prepare_battle_recovery` before their gameplay checkpoint;
unprepared consciousness/injury APIs reject without mutation. No check creates a
random stream mid-injury. Invalid ready records with active countdowns are rejected.

Tests cover first tactical injury replay after a forced recovery-row write failure,
first character roll after restart, idempotent setup, rejection of unprepared
checks, and startup initialization/preservation for both characters and pilots.


### Immediate damage balance consequences

The shared tactical impact traversal now handles first-gyro and leg-actuator
balance checks immediately after the slot is destroyed, before selecting another
critical. The check uses the newly derived piloting penalty. A second gyro loss
or newly destroyed leg forces a one-level fall without a balance roll. Arm
actuators do not trigger these checks; lower/upper/foot losses in a leg with an
already destroyed hip do not add another check. Destroyed and already-prone units
do not fall again. Forced falls still perform the separate pilot-protection check.

`BattleTacticalImpact` and `BattleSalvoGroup` include ordered `balance` reports with
the cause, optional check and optional completed fall. Fall notices and nested
injury notices are included in the enclosing group's notice list. `SectionLost`
remains an explicit notification/casualty item; callers must not apply a second
fall for an already handled tactical loss. Raw material APIs retain their explicit
pending effects and never change posture.

Tactical impacts and salvos take `BattleFallRules`; direct shots supply the same
hit/piloting/toughness choices, including `BattleShotRules.extended_piloting`.
A direct salvo recomputes target-relative hit direction and partial cover before
each group, so a fall's new heading/posture affects later missiles. Head injury
precedes its hit's material phases, while ammunition-explosion injury follows the
nested explosion and any fall it causes. This ordering preserves the pilot's
condition and the unit dice stream at each subsequent check.

Errors, including a required unsupported ice/bridge fall, discard the whole
impact, salvo or shot candidate. Tests cover all four leg actuators, arm and
broken-hip exclusions, successful/failed first-gyro checks, a forced second-gyro
fall within one hit, leg loss, repeated prone damage, explosion/fall injury
ordering, later-missile facing, restart replay and forced database write failure.
Reference evidence: `combat/crit_mechs.c`, `combat/environment_damage.c`,
`combat/mech_ammunition_explosion.c` and `combat/mech_damage.c`.


### Prone turning and stand countdowns

`heading` accepts pivots for running, controlled bipeds while prone, rising or
recovering from a failed stand. The normal damage-adjusted stationary turn rate
applies; position, speed and desired speed remain unchanged. Zero mobility still
prevents turning. Both stand timer variants require zero actual/desired travel
speed but permit a pending heading, so committed turns and remaining countdowns
can survive restart together. Forward/reverse movement and a second stand attempt
retain their existing guards.

Reference: `movement/mech_move_controls.c::mech_heading`,
`movement/mech_update_motion.c::mech_heading_update`,
`unit/mech_lifecycle.c::mech_maybe_move` and `unit/mech_events.c::mech_move_event`.
The heading path has no prone or stand-event exclusion; the speed path explicitly
blocks those states. Tests exercise all three postures/countdowns, both turn-rate
modes, unchanged position/dice, restart continuation and damaged-unit rejection.


### Prone weapon support

Mechanical readiness now includes `posture_ready`. A prone biped cannot fire leg
weapons. An arm weapon needs its opposite arm to survive and have no recycling
weapons; head and torso weapons need either available arm. Individual actuator
losses do not prevent propping while the arm section survives. Every weapon timer
in the supporting arm counts, not just the selected weapon's timer. Firing several
weapons in one arm remains possible while the opposite arm is free; firing that
opposite arm is blocked until the first arm finishes recycling. No separate prop
selection or persistent support timer is needed.

Both expenditure and direct-shot resolution enforce this readiness, including
misses. Prone aim keeps its +2 movement modifier and current lower eye height.
Standing restores ordinary readiness immediately; a rising timer still supplies
the standing-up aim modifier. Physical limb recovery also blocks weapons mounted in that limb and prevents an arm from supporting prone fire. Carried clubs and quad rules remain unfinished. Prone shooters or targets below the water surface
are rejected by the direct-shot vertical-combat guard before expenditure.

Tests cover prone shots, opposite-arm blocking, same-arm firing, torso alternatives,
section/actuator losses, a non-selected recycling weapon, restart and recycle
expiry, leg mounts restored by standing, and submerged-shot rollback. Reference:
`combat/mech_fire_command.c` and `unit/mech_identity.c::sect_has_busy_weap`.


### Damage-induced stagger

Tactical impacts record each incoming damage group once, before material damage.
Transfers and internal ammunition cascades do not add records. Prone units do not
accumulate stagger damage; a fall clears the accumulated damage. Structural or
pilot destruction and administrative placement/removal clear the saved tracker.
Raw material-only APIs still leave gameplay composition to their caller.

`BattleStaggerMode` selects traditional rules for configuration zero, consumed
rolling history for two, and retained rolling history for other nonzero values.
Traditional rules check at twenty accumulated damage, with piloting modifier +1,
then allow at most one check per thirty committed seconds. Sub-threshold damage
expires at a turn boundary. The unit's phase and last-check phase are saved.
Running units maintain their own phase; this replaces reference global event-tick
alignment with a durable unit-local cadence. An inactive conscious unit does not
perform traditional checks.

Rolling modes keep individual incoming groups for sixty committed seconds.
`newstaggertime` controls the interval between checks, with a minimum of one
second. At least twenty uncounted damage is required. Groups are marked or removed
whole until the threshold is covered, so a 13+14-point pair consumes all 27 points.
Retained counted damage adds to later difficulty without causing another check by
itself. The modifier is `floor(damage / 20) - 1`, optionally adjusted by tonnage:
+1 through 35 tons, zero through 55, -1 through 75, and -2 above 75. Stopped units
use modifier 999 and fail without a unit roll. Failed checks resolve a one-level
ground/water fall with the pilot's saved Toughness advantage.

The server schedules these checks in the same world transaction as other btech
updates. Failed persistence restores history, timing, dice and falls together and
discards notices. Lua `unit.state.stagger` exposes a detached history/cadence
snapshot. Histories are bounded to 4096 positive, ordered groups with remaining
lifetimes in 1..60; additional hits beyond that limit reject atomically.

The rolling heartbeat and expiry routines in the reference tree have no callers.
The rewrite deliberately schedules the configured rolling modes, activating that
otherwise dormant rule path. Native history expiry, check timing and traditional
phases survive restart and pause through downtime. In-character casualty handling remains unsupported; an unsupported fall causes
its entire candidate tick to roll back rather than dropping accumulated damage.
Player firing is still not exposed while those and other combat rules are pending.

Tests cover whole-group accounting, retained versus consumed difficulty, thirty-
second check spacing, saved cadence, expiry without extra rolls, threshold and
tonnage behavior, fall clearing, non-duplicated transfer damage, bounded history,
unsupported-fall rollback and live-server retry after a failed fall save.
Reference: `unit/mech_stagger.c`, `unit/mech_events.c`,
`movement/mech_update_damage.c`, `movement/mech_update_heartbeat.c` and
`combat/mech_damage.c`.


### Water falls and persistent flooded sections

`resolve_battle_fall` now uses `levels * (tons + 5) / 20` for water below surface
level and for high-water terrain, with integer truncation before special-map
gravity. Depth-zero water still uses the ordinary `/10` divisor. High-water terrain
uses reduced fall damage but is not itself a section-flooding terrain in the
reference. Ice fracture and bridge surface selection are described below.

`flood_battle_unit` checks all sections atomically. Tactical damage checks the
impacted section immediately after a material phase, including armor depleted
exactly to zero. Standing depth-one units expose only legs; prone units and those
at depth two or deeper expose every section. Front armor zero or originally
present rear armor depleted to zero admits water. Destroyed or already flooded
sections are skipped. A fall checks pre-existing breaches after becoming prone,
before direction and damage groups, then damage phases check newly opened holes.

Flooding is a saved set separate from destroyed critical slots. It disables
mounted weapons, empties ammunition without an explosion, removes explicit heat
sinks and jump jets, and disables engine slots. Enough engine loss destroys the
unit even though its structure remains. Ordinary critical selection can still
hit disabled slots. Flooded legs count as lost support for speed, piloting and
standing; a newly flooded standing leg forces a fall. A surviving opposite leg
allows a stand attempt. A flooded hip also suppresses extra lower-actuator balance
checks. Destroying the section clears its flooded marker; leaving water does not
restore equipment. Drainage and repairs remain later repair-system work.

`BattleSectionExposureReport` and fall/group reports retain new flooding events and their
nested falls. Notice collection includes flooding and nested injuries exactly
once at the enclosing commit. Lua unit inspection exposes `flooded_sections`.
Flooding preserves the distinction between disabled equipment and destroyed
criticals. A flooded head destroys the unit in both tactical and character modes,
without turning disabled cockpit/sensor slots into destroyed criticals. Character
occupant evacuation is published by the flooding action.

Tests cover water depth and damage division, standing after water falls, immediate
leg flooding with preserved structure, support loss and disabled-hip checks,
rear-armor breaches, ammunition/jump-jet loss, engine shutdown, external sink
loss, flooding before fall damage, idempotence, restart and leaving water, and a
live stagger-triggered water fall retried after failed persistence. Reference:
`movement/mech_flooding.c`, `movement/mech_falls.c`,
`combat/environment_damage.c`, `combat/mech_armor_damage.c`,
`unit/mech_systems.c` and `unit/mech_equipment_state.c`.


### Direct-shot glancing rules

The native direct-shot resolver accepts `BattleGlancingMode::from_setting` for
`glancing_blows`: zero disables glancing, two allows a glancing hit one below the
ordinary target number, and every other value makes an exact ordinary hit glance.
The aim subtotal and reported target number remain the ordinary values. The shot
report explicitly identifies glancing hits; its `notices()` includes the target's
glancing message followed by all damage consequences, for transactional delivery.

Medium lasers and AC/20s halve damage, rounded up (three and ten respectively).
SRM/4, SRM/6 and LRM/20 retain full damage per missile, reducing the rolled cluster
value by four instead. Adjusted values below two hit with one missile. The report
preserves the unmodified cluster roll, and grouping proceeds normally. Glancing
spends full ammunition, heat and recycle, introduces no additional dice, and uses
the same atomic damage/fall/flooding path. Out-of-range shots remain misses even
in the more permissive mode and do not consume target damage dice.

Behavioral evidence: `combat/mech_fire_resolution.c` passes the adjusted target
number into hit resolution; `combat/mech_hit_resolution.c` rounds direct damage;
`combat/mech_combat_missile.c` adjusts the cluster table and handles the one-missile
floor. No C code or FFI is included. Tests cover all supported cluster outcomes,
exact hit/miss boundaries in both modes, full expenditure and saved-state replay.
The cockpit command uses this native shot API. Advanced ammunition/malfunctions
and the remaining duel rules are still pending.


### Cockpit direct firing

`weapons` lists current zero-based resolved weapon numbers, mounting sections,
mechanical readiness, recycle seconds and available matching ammunition salvos.
It is a read-only cockpit display for conscious occupants, including passengers
and units with the engine off. Numbers match `fire` and the native/Lua weapon
indices. Wizard template loadout inspection retains its one-based display.

`fire <number> [#unit]` dispatches the native atomic shot resolver. Without an
explicit target it uses the selected lock; an explicit target leaves selection
unchanged. The resolver enforces current contact visibility, arcs, readiness and
the conscious assigned pilot. Configuration supplies glancing, extended ranges,
movement/turning modifiers, gunnery/piloting, hit arcs, critical/stun and stagger
rules. Target Toughness is read from the assigned pilot's character values.

Cockpit firing feedback includes the ordinary aim subtotal, roll and hit/miss or
glancing outcome. The target receives a firing warning; it names the attacker
only if its own current acquired contacts include the shooter, otherwise reporting
the incoming bearing. Damage notices follow. World changes and messages share the
normal command checkpoint and durable save; failure publishes neither. There is
no additional C runtime or FFI. Inspection and invalid requests consume no dice.

This enables the supported direct-fire loop, but does not complete the duel gate:
XP, malfunctions, IC casualty rules, advanced terrain
and equipment still require implementation. Hex fire, short target identifiers,
observer combat broadcasts and aimed-section commands are also pending.


### Triggered ammunition explosions

`BattleUnit::ammunition_hazard_maximum()` identifies the available bin with the
largest remaining internal damage potential. Salvo count is multiplied by full
weapon damage (including the complete missile rack). Equal hazards retain the
first resolved section/slot; empty, destroyed and flooded bins are excluded.
Selection is read-only and consumes no dice.

`explode_battle_ammunition(world, unit, bin_index, fall_rules)` detonates an
available nonempty bin in a private candidate world. It empties and destroys the
slot before applying internal-only damage, transfers and secondary criticals.
It uses the same cascade as critical-hit ammunition explosions, including immediate
leg-loss falls and section flooding. It does not roll an ordinary hit location or
count the explosion itself as incoming stagger damage. Invalid indices, empty bins,
unsupported casualty/terrain effects and validation failures publish nothing.

Both trigger paths now stage the ammunition-explosion notice and, for a surviving
assigned pilot, the personal-injury notice. Explosion injury is two tactical hits,
or one with the canonical `Pain_Resistance` advantage set to a nonzero value.
Fall injuries occur before the explosion injury, as with other nested damage.
Reports carry all notices and consequences for the enclosing transaction.

Behavioral references are `unit/mech_ammunition.c` (maximum-bin selection),
`combat/mech_ammunition_explosion.c` (internal damage and injury), and the existing
critical/transfer contracts. Tests verify selection ties, unavailable bins,
internal damage, save/reload replay, injury reduction, fall composition and atomic
ice-terrain rejection. Periodic heat hazards now use this common operation.


### Periodic overheating and reactor shutdown

The heartbeat now calls `advance_battle_overheat` after heat accounting and
existing recovery countdowns, before stagger resolution. Thermal clock changes,
unit dice, personal injuries, explosions, falls and cockpit messages commit
atomically with the rest of the tick. A failed save restores the original check
and its random stream; retry produces the same result. `unit.state.overheat_clock`
exposes the saved counters without exposing dice.

The clock deliberately uses unit-local committed samples rather than the C
server's unsaved global event-tick phase. Running units, units that are still
cooling, and stable units with at least ten excess heat advance it. A newly
constructed unit starts at zero. The injury phase runs every thirty samples;
ammunition/shutdown checks become eligible after thirty samples and remain ready
while excess heat is below ten. A performed hazard check resets that elapsed
counter. Both clocks pause offline and resume on restart. Destroyed units clear
the clock. Thermal accounting and hazard consumption are separate domain APIs;
the server always calls both in the same transaction.

At an injury boundary, damaged life support causes one tactical injury from
15 through 25 excess heat and two above 25. With intact life support, heat above
30 instead rolls a coin for one injury. This precedes ammunition and shutdown
checks. Ammunition avoidance uses 2d6 targets 4/6/8 at excess heat 19/23/28; a
failed roll detonates the most destructive available bin. The roll is still
consumed when no ammunition remains, with the corresponding cockpit notice.
A unit destroyed by injury or explosion receives no subsequent shutdown check.

Assigned player pilots use the mental `Computer` target plus 0/2/4/6/8 at heat
14/18/22/26/30. A nonzero effective skill rolls 2d6; an unskilled pilot rolls three
d6 and keeps the lowest two. Computer overrides remain possible above heat 30,
including for disconnected assigned player pilots, as in the reference's player
branch. The cockpit sees the override notice, modified target and roll. Computer
XP awards remain deferred with the other skill-XP integration.

Unpiloted units use shutdown targets 4/6/8/10 at 14/18/22/26, and shut down without
a roll at 30 or above. The reference also shuts down an unpiloted running reactor
at checkable heat 10 through below 14 without rolling; that behavior is preserved.
An assigned player below 14 requires no override and stays running.

Shutdown clears the pilot assignment, selected target and stand countdown, and
stops translation and pending turning. Above an absolute speed of 10.75 kph, a
standing unit first makes a +3 piloting check. Failure performs the reference's
zero-multiplier fall: pilot protection, prone posture, flooding and random heading
still apply, but no structural fall packets are added. The ordinary public fall
API continues to reject a zero multiplier. Unsupported ice/bridge falls reject
the complete thermal candidate. Prone and slower units shut down without this
balance check. Airborne falls, IC casualties, infernos, cutoff and advanced heat
sink modes remain outside supported construction/gameplay.

Evidence: `movement/mech_update_heat.c`, `movement/mech_overheat.c`,
`movement/mech_falls.c`, `unit/mech_lifecycle.c`, and the character skill-roll
contracts. Tests cover cadence/restart, exact hazard bands, skilled/unskilled dice,
injury-before-explosion ordering, the intact-support coin, speed-boundary falls,
clock validation and a live-server failed-save/retry of shutdown.


### Lua firing and weapon inspection

Trusted game callbacks now use `btech.unit.weapons(unit)` for detached weapon
inspection and `btech.unit.fire(unit, pilot, weapon_index, target)` for configured
direct fire. The weapon list is a Lua array; each row's `index` is the zero-based
number accepted by both native and Lua firing. The row includes display name,
mounting section, rear mounting and mechanical readiness. An omitted or nil
target uses the selected lock; an explicit target leaves selection unchanged.

The native command and Lua function share the same rule mapping, target checks,
shot resolution and cockpit message construction. The binding applies damage,
ammunition, heat and recycle and stages all messages automatically. Its detached
shot report contains the aim, rolled result, expenditure and optional salvo with
nested damage/fall/flooding reports. Callers must not apply those effects or emit
the notices a second time. No private random stream is exposed.

Each Lua fire operation has its own world/effect checkpoint. An operation error
caught by Lua leaves that operation unapplied; a later uncaught callback error
rolls back the complete callback. A durable save failure rolls back the enclosing
command, including messages. Both functions require callback scope and reject
checking mode. Trusted scripts own permission to act as a supplied pilot; the
shared resolver still requires that pilot to be assigned, present and conscious.

All BattleTech detached serialization now represents absent optional values as
Lua `nil`, rather than a truthy null userdata. This includes missed salvos,
non-ammunition weapons' bin indices, cleared locks and absent pilots/positions.
Both copies of the Lua type definitions describe these contracts. Tests compare
native and Lua outcomes/messages, detached tables, nil results, callback rollback,
protected operation errors, checking guards and live save failure/retry through
both command paths.

The reference's combat, piloting and Computer XP awards require in-character
units. Tactical simulator actions therefore correctly award no XP. In-character
progression remains part of the persistent character-system gate; it must not be
implemented by granting experience for these simulator actions.

## Connected duel acceptance scenario

`tests/btech_duel.rs` starts two TCP clients on an isolated flat battlefield with
the unchanged AS7-D and JR7-D test templates, opposing teams, character skills
and seeded unit/recovery dice. Both players enter their units, claim the cockpit,
complete normal startup, acquire optical contacts, settle target locks and fire
using native commands until one unit is destroyed. Both units must take damage
and both clients must receive incoming-fire feedback. All gameplay mutations
after fixture setup go through the clients and the ordinary server heartbeat.

Tokio's test-only clock advances one second at a time, waiting for each heartbeat
to commit before continuing. It does not replace startup, acquisition, lock,
heat or recycle processing. A normal speech command fences each command response
so a rejected shot fails with its actual feedback instead of waiting for a hit.

The scenario checks destruction cleanup without moving or deleting the players,
restarts the server, and verifies that the wreck remains destroyed and rejects
startup and firing even after an explicit new cockpit claim. This establishes a
connected tactical duel, not the remaining jumping, terrain-transition, observer
broadcast or complete combat-interface coverage required by the mech-duel gate.

### Direct-fire destruction feedback

A lethal direct shot concludes its damage feedback with `You destroyed the
target!` to the firing cockpit and `You have been destroyed!` to the target
cockpit, matching `combat/mech_combat_misc.c`. A shot reports this outcome once
even when it contains multiple missile groups or nested critical/fall damage.
Misses and surviving hits do not announce destruction. These messages belong to
the shared shot report, so native commands and Lua firing use the same routing
and transactional publication. Failed saves discard them along with damage and
pilot-release state; a successful retry announces the result once.

The connected duel asserts both messages reach the appropriate cockpit exactly
once. Native and Lua save-failure scenarios cover lethal and nonlethal shots,
including a response fence that detects delayed or duplicate output. Observer
broadcasts, explosion artwork and standalone environmental destruction feedback
remain separate interface work.

## Jump capacity and trajectory foundation

`BattleUnit::jump_capacity(gravity)` derives conventional thrust from the template
jump speed, subtracting 10.75 kph per effective destroyed or flooded jump jet.
Repeated damage and overlapping physical/flood losses count once. Destroyed units
have zero capacity. Map gravity scales thrust by `100 / max(gravity, 50)` and the
range allowance is the resulting speed divided by 10.75, truncated to whole
movement points. Gravity must be within the saved map's 0–255 range. Gravity
applies independently of the map's special-rules flag, as in the reference map
condition assignment and jump-speed functions.

Lua `btech.unit.state(id).jump_capacity` returns detached `speed` and
`movement_points` values using the current map gravity (100 for an unplaced
unit). It describes capability, not authorization to jump. Existing damage and
flood facts persist the result without another mutable capacity counter.

`BattleJumpPath` owns immutable takeoff/destination points, endpoint elevations,
distance and apex. Horizontal distance and absolute elevation change must each
fit the supplied movement-point allowance. The additional apex elevation is
`floor(min(MP + 1 - distance/3, 2*distance + 2))`. A sample interpolates endpoint
heights and adds a quartic arc; it switches to a quadratic arc when the stored
apex exceeds current movement points after lost thrust. Takeoff and landing
samples return their exact endpoint coordinates. Progress outside 0–1 and
nonfinite input are rejected.

Reference evidence: `movement/mech_jump.c`, `movement/mech_jump_speed.c`,
`movement/mech_motion_integration.c`, `combat/crit_mechs.c`, and
`map/map_conditions.c`. `tests/btech_jump.rs` checks gravity boundaries, damaged
and flooded jets, endpoint/apex geometry, the curve change after lost thrust,
invalid inputs, persistence and detached Lua inspection.

The connected runtime below uses this foundation for clear level routes.
Obstacle handling, special-terrain landings and airborne combat remain incomplete.

### Restartable flight progression

`BattleJumpFlight` adds a serializable flight cursor to the trajectory foundation.
It records completed horizontal distance and the movement points used for the
last airborne sample. Changing thrust between ticks therefore leaves the last
sample unchanged; the next step uses the new thrust and curve. The launch path
serializes only its original endpoints, elevations and movement-point allowance,
then reconstructs and validates distance/apex when loaded. Invalid progress,
out-of-range destinations, fractional endpoint elevations and unknown serialized
fields are rejected.

Each `advance` represents one committed second. Distance advances by current
gravity-adjusted kph divided by 645, multiplied by the map movement percentage;
nonpositive percentages use 100. This follows the reference jump event's
one-second cadence and the same normalized movement scale as ground motion.
A full five-MP, five-hex jump takes 60 such steps at normal gravity and movement
rate. Overshoot and accumulated rounding error snap to the exact destination.
An arrived cursor rejects another advance without changing state.

The returned `BattleJumpStep` includes previous/current airborne samples and an
`airborne`, `landing` or `lost_thrust` outcome. Zero thrust leaves the cursor at
its previous airborne position and requests lost-thrust handling; it does not
teleport to the destination. Invalid capacity inputs do not change progress.
The enclosing world transaction must handle collisions, landing or falling
before committing a step.

Tests cover altered gravity/rates, exact arrival, mid-flight and completed JSON
round trips, changing thrust, invalid records, zero thrust and overshoot. The
connected runtime stores this cursor directly on the unit and advances it in
the ordinary server transaction.

### Connected terrain-aware jumping

Native `jump <bearing> <range>` and Lua `btech.unit.jump(unit, pilot, bearing,
range)` launch the same domain operation. The projected destination snaps to
its hex center. As in the reference, admission and apex calculation use the
requested range before snapping; the actual flight to the center can be slightly
longer. The stored projection is rechecked against its destination when loaded.
Supported routes include grassland, roads, light/heavy forests, rough ground,
mountains, snow, smoke, fire and water tiles. Underground maps reject launch. A conscious, assigned,
present pilot and running, standing unit are required. Standing/stabilizing
countdowns, an existing flight, nonfinite/nonpositive
range, same-hex destinations and unsupported routes reject without mutation.
DFA arguments remain unsupported.

Units persist `flight` and `jump_stabilization`; loading verifies their power,
posture, motion, coordinates and supported map route. Ground motion skips
airborne units. Launch sets actual ground speed to zero while preserving the
desired speed for resumption after landing. Ground speed/heading commands and
voluntary shutdown reject during flight. Administrative placement clears a
completed stabilization timer; it still requires shutdown first.
Normal and thermal shutdown cancel any remaining stabilization countdown.

The heartbeat samples jump heat before advancing flight, then commits motion,
hex identity, falls and notices with the rest of its one-second update. Flight
produces at least three heat, or one per surviving unscaled jump MP, in addition
to engine heat. Normal landing clears flight and starts twelve stabilization
seconds. Missing pilots, unconsciousness or lost thrust cause a one-level fall;
damaged legs/gyro require the existing piloting check before landing. Destruction
cancels flight and stabilization. Thermal shutdown during flight applies a fall
before powering down. Administrative jet losses are processed on the next committed
flight tick. Tactical loss of the last jet applies its one-level fall immediately
inside the damage transaction, including the jet shutdown and falling notices.

Range includes airborne height, and terrain LOS uses it for endpoint eye levels
and excludes ground partial cover for airborne targets. Conventional direct fire
uses this geometry for airborne shooters and targets, with the same contact,
arc, readiness and transaction requirements as ground fire. Stabilization contributes the reference's +2
attacker movement modifier after landing. Airborne turning,
bridge landings, under-ice travel, building/wall interactions, DFA and observer
broadcasts remain required for complete jumping coverage.

Lua unit state exposes detached `flight`, `airborne` and `jump_stabilization`
values. Launch notification and state use the callback/command checkpoint.
`tests/btech_jump.rs` covers native/Lua parity and rollback, midpoint height and
LOS over a hill, preserved desired ground speed, landing and stabilization,
lost jets, destruction, thermal shutdown, malformed saved state, and live TCP
launch/heartbeat save failures followed by retry and a mid-flight server restart.


### Airborne damage, posture and firing

Detached aim inspection uses current gravity-adjusted jump capacity for target
movement bands and adds the jumping target's +1, even though actual ground speed
is zero. The airborne attacker contribution is +3; post-landing stabilization
remains +2. Inspection consumes no dice and does not grant firing permission.

The tactical impact resolver checks the first gyro hit immediately. Failure
causes a fall whose multiplier is the current gravity-adjusted whole jump MP;
a second gyro hit forces that fall without another balance roll. A zero whole-MP
multiplier still changes posture and rolls pilot protection, but deals no fall
structural damage. Remaining jet hits reduce thrust; losing the last unscaled
jet forces a one-level fall. Leg actuator hits defer their balance check while
airborne and leave the damage available to landing checks. Traditional stagger
checks consume qualifying damage without a balance roll during flight; rolling
stagger retains its existing heartbeat behavior.

These are shared Rust domain rules, with no bridge or separate combat state.
Seeded tests cover selected actuator/jet/gyro criticals, successful first-gyro
checks, forced second-gyro falls, gravity and zero-MP fall multipliers, and saved
RNG replay after reconnecting the pilot. A single missing leg leaves the unit upright during flight and checks balance
at landing. Loss of both legs or hips marks the unit prone while the trajectory
continues; posture resets facing and rolling stagger history without adding fall
damage or consuming dice. Saved prone flights require that structural condition.
Standing is unavailable before landing. Prone landing preserves the normal
already-fallen piloting success rule. Losing the last jet still ends a prone
flight with a fall. Landing notices distinguish missing legs, actuators and gyro.

Native and Lua fire now support jumping shooters and targets through the ordinary
shared resolver. Tests cover either participant or both jumping, real acquired
contacts, movement modifiers, expenditure and damage, callback rollback, and
native/Lua parity after a saved flight reload. No additional firing bridge or
parallel damage engine is used. Obstacles,
special terrain, DFA and observer broadcasts remain incomplete.


### Pilot-requested early landing

Native `land` and Lua `btech.unit.land(unit, pilot)` abort a jump at its current
horizontal point. Both require the conscious assigned pilot to be present. The
initial piloting check consumes saved unit dice. Success enters the same landing
resolver as natural completion, including a second check for damaged landing
gear; failure applies a one-level fall. Either result ends flight and starts
stabilization for a surviving unit. Posture, damage, injuries, dice and ordered
notices commit together. Invalid requests consume nothing.

Tests cover success and failure, the second damaged-actuator check, detached
native/Lua parity after restart, callback rollback and a real TCP save failure
that preserves the airborne cursor and dice before a successful retry.


### Jumping with accumulated damage

Ordinary damage history does not block launch or add a separate landing roll.
The reference's `MechConditionSummary.staggering` reads `rd.stagger_damage / 20`,
whereas ordinary damage appends rolling records or increments turn damage. The
source currently has no positive update to that separate counter: its only
assignments reset it to zero or the old event's negative buffer. The Rust engine
does not introduce another counter or equate rolling records with that flag.

Launch and landing leave the existing history to the stagger heartbeat.
Traditional checks consume qualifying damage without a control roll during
flight; rolling Retain/Consume checks continue and can force a one-level fall.
Tests cover below-threshold and qualifying histories in all three modes, saved
flight replay without extra dice, native/Lua launch and rollback with pending
damage, and rolling falls after takeoff.


### Elevation and hill collisions

Grassland and road jumps use the destination terrain height in the saved path.
Ascent and descent must both fit within current whole jump MP. Paths retain their launch-time endpoint heights even if ice later breaks;
terrain reload continues to require removing units first. Launch below elevation -1 is rejected.

On entering a new hex, rounded jump height is compared with its surface. An
obstructed transition leaves the unit at its previous horizontal point. An active
pilot attempts a control check using the previous surface height divided by
three, truncated toward zero. Success uses the shared landing resolver; failure
or no active pilot causes a one-level fall. Notices, dice, landing state and
flight cancellation commit together. Checks cover mapped hills, buildings and walls. Subsequent terrain sections
describe water, ice and bridge interactions.

Tests cover uphill/downhill limits, saved height replay, and safe/crashed obstacle
landings with identical results after restart.


### Dry terrain flight coverage

Forests, rough ground, mountains, snow, smoke and fire tiles now use the same
jump surface-height and landing resolver as grassland and roads. Terrain speed
penalties remain ground-motion rules; they do not slow jump thrust. Trees do not
add a separate canopy collision or landing roll. Existing LOS/sensor rules still
apply to contacts and firing. Fire tiles contribute five heat in addition to
jump heat, including while airborne, matching the reference heat updater.

Parameterized tests cover native/Lua parity, full routes, midpoint persistence,
landing replay without extra dice, and airborne/landed heat rates for all seven
newly admitted terrain types. Water/ice immersion, bridge clearance, building
and wall interactions remain separate work.


### Water flight, immersion and landing

Water routes now use bottom elevation for destination height and the existing
ascent/descent limit. Takeoff below elevation -1 remains unavailable. Water does
not act as a hill collision. Cooling uses current rounded jump altitude, so
flying over deep water gains no immersion bonus. Newly breached sections use
current depth; whole-unit immersion on hex entry and landing uses bottom depth
once the unit is below the surface. Existing flooding, equipment loss and fall
rules apply in the enclosing flight transaction. Flooded legs do not cancel
airborne thrust; loss of both supports marks the unit prone during flight.

Ordinary firing above deep water now checks airborne altitude instead of the
map's bottom depth. Underwater firing restrictions remain. Tests cover shallow
takeoff, deep takeoff rejection, airborne firing, absence of flooding/cooling
above water, landing flooding and cooling after restart, and submerged hex-entry
flooding while thrust continues.


### Atomic ice-fracture domain operation

`break_battle_ice` converts one ice tile to water at the same depth, then resolves
water falls for surface occupants in deterministic unit order. A supplied
triggering unit falls after its neighbors. Fall multipliers use the original ice
depth; zero-depth ice changes terrain without fall rolls. Each unit's Toughness
setting is applied independently. Terrain, flooding, injuries, damage, dice and
notices belong to one candidate world; an error discards the complete fracture.

Persistence admits this specific Ice-to-Water transition on occupied maps and
keeps the existing terrain dictionary codes stable. Arbitrary terrain reloads
still require removing units first. Tests cover two-unit fall order, restart
replay, zero-depth behavior, a late casualty rejection, and a unit-write failure
that rolls back both the changed tile and every affected unit.

This is the shared resolver for subsequent landing, weapon and environmental
triggers. Landing and fall triggers are described below. Weapon/environmental triggers
and below-ice travel remain incomplete; there is no new player command or Lua
mutation binding for this low-level operation yet.


### Intact ice surfaces and automatic landing checks

`BattleHex::standing_height` distinguishes the intact ice surface at zero from
its stored water depth. Unit range, LOS endpoints, jump launch/destination heights
and immersion use the standing surface. Jump routes can include ice, and normal
firing on that surface uses the ordinary contact/arc/readiness rules. Ground
walking/skidding and travel underneath intact ice remain incomplete.

Landing and falling onto ice roll one saved unit d6; one fractures the tile.
The shared fracture resolver applies neighboring falls before the triggering
unit. Fall reports retain an optional nested `ice_break` report, and subsequent
fall damage uses the resulting terrain (water halves structural fall damage).
Ice already changed to water cannot recursively fracture again.

Flight paths preserve their launch-time heights when another unit breaks the
destination ice. Natural and early landings use the current tile when touching
down. Seeded tests cover ice holding/breaking, nested falls, mid-flight terrain
changes and restart, and native/Lua early landing including callback rollback of
the changed tile and every neighboring unit.


### Atomic bridge collapse and retained altitude

`break_battle_bridge` shares the terrain-break transaction and
`BattleSurfaceBreak` report with ice fracture. A bridge becomes depth-one water.
The reference changes the map before selecting casualties: its elevation getter
reads the replacement depth, so only occupants at elevation one fall, using a
two-level multiplier. Occupants at other elevations retain their altitude. This
ordering is explicit here; it does not implement a deck-height-plus-one fall.
The behavioral evidence is `movement/mech_ice.c` (`break_sub`, `swim_except`),
`map/map_terrain.c` (live elevation access), and `map/map_terrain_updates.c`
(terrain notification does not reposition units) in the read-only reference.

Units can retain an explicit non-flight altitude after the terrain changes.
Range, LOS endpoints, immersion and jump launch use it. Falls and administrative
placement clear it; launch transfers the height into the saved trajectory.
Shutdown preserves it. This is narrow support for terrain destruction, not full
under-bridge movement or bridge jumping.

Occupied-map persistence accepts Bridge-to-Water at depth one as well as
Ice-to-Water at its original depth. Terrain, casualties and unit altitude remain
one database transaction. `tests/btech_surfaces.rs` covers bridge heights zero,
one, three and nine, restart replay, failed casualty resolution, failed database
writes, and retained altitude through falls, shutdown, placement and later jumps.
Automatic weapon/environmental collapse triggers and bridge traversal remain
pending.


### Optical acquisition and firing on intact ice

The shared terrain LOS query and automatic scanner admit intact ice endpoints.
Surface units stand at elevation zero regardless of water depth, and airborne
units keep their trajectory height. Ice does not make a unit underwater merely
because its tile stores a depth. Intervening ridges and the water/air boundary
continue to block sight normally.

Tests cover surface depths zero, one, three and nine, sight in both directions,
terrain occlusion and submerged targets. Native and Lua shots acquire actual
optical contacts on ice, including while jumping, replay identically after
restart and restore their complete state on callback failure. Travel below intact
ice still needs further vertical rules.


### Standing on intact ice

Surface units can attempt to stand on intact ice. Successful rises keep the ice
intact; a failed skill check enters the existing fall resolver, including its
one-in-six fracture check and ordered neighboring falls. All damage, terrain,
dice, notices and the recovery countdown share the enclosing transaction.
Standing below the ice surface still requires the separate breakout behavior
and is rejected before consuming dice.

The shared native/Lua adapter applies the configured careful-standing and
casualty rules once. Seeded tests cover normal, careful and anyway attempts,
success, intact-ice failure and fracture, callback rollback of neighboring units,
and restart through both rise and retry countdowns. The read-only behavioral
reference is `movement/mech_move.c::mech_stand`.


### Optical water attenuation uses target altitude

Both optical query paths use the target's current integer altitude and posture
when applying the visual sensor's six-water-hex limit. A tile's stored depth is
not evidence that a jumping unit or terrain-collapse survivor is underwater.
The test covers prone survivors at elevation zero across six shallow-water hexes,
restart, and the visibility change when placement settles them onto the bottom.
The read-only reference is `sensors/mech_sensor_functions.c::vislight_csee`,
which checks target base altitude and posture-adjusted eye height.


### Bridge deck optical contacts

Terrain LOS and the automatic scanner admit units on bridge decks, using deck
height for ordinary placement and explicit altitude when present. Bridge spans
retain the reference exception to solid-ground LOS blocking. Ridges, woods and
the air/water boundary still apply. This supplies visibility and contact queries;
bridge traversal remains pending; deck standing and combat falls are described below.

Tests cover deck heights zero, one, three and nine, intervening spans and woods,
underwater sight beneath spans and rejection across the waterline. Native and
Lua contact lists agree. Saved contacts refresh after bridge collapse: retained
altitude preserves visibility, while a prone unit that falls underwater loses
contact with its deck observer. Restart replays those transitions identically.
Read-only reference: `sensors/mech_los.c` and `sensors/mech_sensor_functions.c`.


### Current altitude inspection

`battle_unit_elevation(world, unit)` returns the signed integer altitude used by
terrain effects, or `None` for an unplaced unit. Lua `unit.state(unit).elevation`
and native `@btech inspect` use the same query. Intact ice reports zero, ordinary
bridge placement reports deck height, terrain-collapse survivors retain their
saved height, and flight uses the committed jump's terrain-effect rounding.
The fractional trajectory remains available in Lua's `airborne.elevation`.

These are read-only projections. Script edits to the returned state table do not
move the unit. Tests cover ordinary terrain, water, ice, bridge collapse/restart,
launch, flight, landing and removal, with agreement between both adapters.


### Bridge falls, standing and direct fire

Bridge falls use the biped's two-level elevation-change limit. A starting altitude
strictly below `deck - 2` settles at elevation -1 beneath the span; otherwise the
unit settles on the deck. Lower-surface falls use water's half-damage divisor,
while deck falls use ordinary damage. Neither operation destroys the bridge.
Standing and direct fire now use these shared fall rules. Existing underwater
posture and submerged-weapon restrictions continue to apply.

Cooling uses actual altitude for water, ice and bridge tiles. Whole-unit flooding
retains the reference's surface-elevation lookup: a bridge's nonnegative deck
elevation prevents that check from flooding sections. A fresh armor breach uses
actual submerged altitude and can flood a section beneath the bridge. This
ordering is intentional behavioral fidelity, not an inferred water-depth model.

Tests cover the exact two-level boundary at deck heights zero, three and nine,
restart replay, native/Lua fire and stand rollback, cooling and fresh breaches.
Existing leg-loss, stagger, ammunition-explosion and zero-damage thermal-fall
scenarios now exercise successful bridge-deck consequences. Bridge movement and
jump route selection remain pending. Read-only reference: `mech_falls.c`,
`mech_landing.c::mech_drop_surface_set`, `mech_collision.c::bridge_set_elevation`,
`mech_flooding.c` and `mech_update_heat.c` under `movement/`.


### Bridge jump collision contract and remaining position transition

`BattleHex::blocks_jump_entry` owns the terrain entry predicate used by the jump
resolver. Water entry proceeds to immersion; ordinary terrain compares altitude
with surface height. Bridge entry collides at negative altitude or exactly one
level below the deck, allowing passage at other heights below a high span.
`strikes_bridge_during_jump` captures the additional pre-transition bridge check,
which only triggers at positive altitude. Boundary tests cover every bridge deck
height supported by map assets.

Bridge routes apply both checks in reference order. A pre-transition collision
updates the continuous point but retains the bridge hex used for fall effects.
The unit records `hex_sync_pending` when those positions differ. Ordinary
position mismatches still fail validation, and a pending update requires zero
travel speed. The next successful movement/jump step synchronizes the hex;
placement and removal also clear it. Ground terrain transitions that are not yet
implemented retain the pending update instead of silently moving the unit to a
different surface. Standing and shutdown can preserve the interrupted state.
The behavioral evidence is
`movement/mech_collision.c::collision_check`,
`movement/mech_motion_integration.c::mech_motion_integrate`,
`movement/mech_update.c` and `movement/mech_update_hex_mech.c`.


### Bridge jump routes and interrupted hex updates

Bridge deck takeoff and destination heights use the ordinary jump capacity and
ascent/descent limits. Routes can pass below high spans, collide while entering
one, or strike its underside during vertical integration. Natural and successful
early landings select the current deck. Loss of thrust resolves its fall before
clearing the committed airborne altitude, preserving lower-surface selection.

`unit.state(unit).hex_sync_pending` and `@btech inspect` expose interrupted hex
updates. The continuous point remains available for range while the retained hex
supplies terrain effects. This state survives save/restart and failed writes,
and units can stand and jump again without administrative repair. No general
exception permits an unmarked disagreement between motion and position.

Tests cover deck heights zero, three and nine, native/Lua launch and early landing
rollback, entry collisions, underpasses, same-hex underside collisions, collisions
that cross a boundary, saved dice replay, failed database writes, recovery/relaunch
and lost thrust below a span. Ground bridge traversal and remaining map-edge
transition behavior are still separate work.


### Level bridge deck traversal

Ground motion admits bridge decks and adjoining supported land at the same
elevation. Forward and reverse travel use the existing acceleration, turning
and segment-traversal rules; no extra piloting roll is added for a level deck.
The unit's actual altitude must match its current standing surface before travel
can proceed. This prevents a unit beneath a bridge or at a retained height from
being silently moved onto a different surface.

Tests cover deck heights zero, three and nine in both directions, native/Lua
throttle parity and callback rollback, midpoint save/restart, and unchanged dice.
A below-deck unit remains at its existing position and altitude when it requests
ground travel. Changes in deck height, ramps, cliffs, water entry and ground
travel beneath spans still require the remaining terrain-transition rules.
Read-only reference: `movement/mech_update_hex_mech.c` and
`movement/mech_collision.c`.


### Forward one- and two-level ground steps

Forward travel on supported land and bridge decks can ascend or descend one or
two levels per crossed hex. Each one-level change subtracts 10.75 kph from actual
speed; each two-level change subtracts 21.5 kph, clamped at zero. Desired throttle
is preserved, so subsequent ticks accelerate normally. Costs apply on entry, not
repeatedly while traversing the same hex. The segment trace checks consecutive
heights instead of comparing every tile with the starting height.

These steps consume no piloting dice. Tests cover ascending and descending land
and bridge routes, exact speed costs, preserved throttle, midpoint restart and
pending hex synchronization. Completing a pending step discards the old altitude
override even when the continuous point itself did not move. Steeper changes use the cliff checks described below.
Reverse steps use the configured checks described below.
Read-only reference: `movement/mech_update_hex_mech.c` and the `MP1`/`MP2`
constants in `unit/equipment_types.h`.


### Reverse ground elevation checks

Reverse one- and two-level steps on supported land and bridge decks now honor
`battletech.roll_on_backwalk`. With checks enabled, the piloting modifier is the
absolute height change minus one. Success keeps actual speed and throttle;
failure applies the shared fall at the destination, stops movement, and rolls
uphill movement back to its prior position. Downhill falls stay in the new hex.
An unassigned pilot bypasses the obstacle check, matching the reference behavior.
With checks disabled, reverse steps use the same speed deductions as forward steps.

Movement ticks now return a result and resolve in a disposable world candidate.
Any failed control/fall operation restores the entire tick, including units that
moved earlier and their dice. The server supplies configured fall rules and keeps
movement in its existing save-and-notify transaction. Seeded tests cover both
height changes and directions, bridge decks, the exact control threshold,
configuration off, uphill rollback, downhill falls, and save/restart replay.
Water entry and below-deck ground travel remain unfinished.


### Ground cliff avoidance and falls

Height changes above two levels on supported land and bridge decks now use cliff
avoidance. Uphill checks occur at the previous position; successful downhill
checks return there. Failed downhill checks fall in the entered hex, with damage
scaled by the drop. Every cliff outcome stops actual speed and throttle.

Ordinary avoidance uses the truncated absolute value of `(speed + 10.75) / 10.75`,
then integer division by three. `battletech.skidcliff` instead selects modifiers
-1, 0, 1, 2 or 4 at the 2.1, 4.1, 7.1 and 10.1 MP boundaries. Ordinary uphill
crashes use the truncated `(1 + speed / 10.75)` divided by four; the skid variant
uses one fall level. The signed ordinary calculation preserves zero or negative
multipliers in reverse: these still roll pilot protection and change posture,
but apply no structural damage. Unpiloted units stop without an avoidance roll.

The first cliff ends the movement trace, even when a map movement modifier would
otherwise carry the unit through several hexes or beyond the map. Rollback retains
any interrupted bridge hex synchronization. Tests cover thresholds, both travel
directions, land and bridge decks, stopped and fallen outcomes, unpiloted units,
large movement increments, and persisted replay. Water entry, below-deck travel and observer broadcasts remain to be integrated.
Read-only references: `movement/mech_update_hex_mech.c`, `movement/mech_skid.c`,
and `movement/mech_falls.c`.


### AutoFall preference

`mechprefs` displays AutoFall; `mechprefs AutoFall [ON|OFF]` sets or toggles it.
Native control and `btech.unit.auto_fall(unit, pilot, enabled)` share the assigned,
present, conscious pilot check and permit a stopped engine. The boolean belongs
to the unit and survives shutdown, cockpit changes and persistence. Lua state and
native inspection report it. Other reference preferences remain unimplemented.

AutoFall skips only a piloted downhill cliff's avoidance roll. The fall still
uses its ordinary pilot-protection and damage rolls. Uphill checks and unpiloted
cliff stops retain their existing behavior. Tests compare the resulting stream
and damage directly with a fall that has no preceding avoidance roll, replay
saved state, and exercise native/Lua parity, callback rollback, invalid input,
unauthorized pilots, toggling and shutdown persistence.
Read-only reference: `unit/mech_advanced.c` and `movement/mech_update_hex_mech.c`.


### Ground water movement

Bipeds can enter, traverse and leave ordinary water using signed bottom elevation.
Map depth does not directly divide target speed in the reference call path.
Elevation costs and cliff checks apply across shorelines. Surface depth-zero water
has no immersion roll or speed penalty. Retained altitude that differs from the
bottom still needs a separate settling transition.

Each entered submerged hex applies a no-XP piloting check with modifiers -1 at
depth one, zero at depth two and +1 at depth three or deeper. Forward throttle is
capped at walking speed. Actual speed above walking plus 0.1 kph adds two to the
check. Failure resolves the shared one-level water fall. A reverse elevation
step uses its own control check and bypasses the water check, matching the
reference early return; immersion still floods breaches after entry. Flooded
legs can force a fall, and subsequent movement does not restore their throttle.

`water_movement.rs` owns these entry effects inside the existing atomic motion
tick. Tests cover preserved target speed in both directions, dry depth-zero water,
submerged thresholds, running entry, failed checks, flooded support, shoreline
exit, unchanged dice within a hex, restart replay and failed-action rollback.
Special high-water terrain, waterproof technology and observer broadcasts remain pending.
Read-only references: `movement/mech_update_speed.c`,
`movement/mech_update_hex_mech.c`, and `movement/mech_update.c`.


### Ground travel beneath bridges

Ground movement now retains the lower bridge surface instead of stopping a unit
at elevation -1. At each entered bridge hex, a biped more than two levels below
the new deck selects elevation -1; otherwise it selects the deck. Movement within
a hex preserves the existing altitude. The chosen surface is stored with each
checked crossing and restored with position on rollback.

Map elevation remains distinct from physical altitude. Step and cliff checks use
the mapped heights, while an elevation speed deduction applies only when the unit
is on the entered map surface. Consequently, a unit can stay below rising spans
without a forward elevation deduction, or climb onto a low deck while traversing
a decrease in mapped height. Exiting a tall span into shallow water can still
trigger the reference cliff check. Below-deck water checks use the depth-one
modifier; the bridge's mapped elevation does not apply a water speed divisor.
Existing whole-unit bridge flooding retains its mapped-depth behavior.

Tests cover forward and reverse travel, both sides of the deck selection boundary,
under-span checks, lower-surface falls, mapped cliff exits, rollback, unchanged
throttle where appropriate, and persistence replay. Special high-water terrain and observer broadcasts remain pending.
Read-only references: `movement/mech_collision.c`, `movement/mech_update.c`,
`movement/mech_update_hex_mech.c`, `movement/mech_update_speed.c`, and
`map/map_terrain.c`.


### Water speed call-path correction

Water terrain does not independently halve or quarter requested speed in the
current reference server. `mech_speed_update` supplies `mech_position_elevation`
to `mech_terrain_speed`. That getter returns the raw map depth, a value from zero
to nine. The terrain helper tests for negative elevation before applying its water
penalties, so neither condition is reached. This differs from the signed surface
height used by movement geometry. Rust now preserves this caller-visible behavior;
water-entry throttle caps, elevation-step costs, heat, turning and acceleration
remain independent effects.

The same input distinction explains the absence of a depth speed divisor beneath
bridge decks. Tests exercise forward and reverse requests at depths zero, one,
two and nine, and shoreline exit without an invented deceleration penalty.
Read-only references: `movement/mech_update_speed.c`,
`unit/mech_position_state.c`, and `map/map_terrain.c` (`mech_hex_get`).


### Ground travel on and beneath ice

Ice entry now separates the existing altitude, the selected surface and mapped
bottom depth. An entry that starts at elevation zero rolls the existing one-in-six
fracture check before ordinary entry rules. Fracture commits terrain replacement,
neighboring surface falls and the triggering fall together. Movement within the
same hex does not repeat the check; depth-zero fracture consumes only the ice die.

A submerged entrant follows the new ice bottom. Starting at elevation -1 and
entering depth-one ice selects the surface, without a fracture roll on that entry.
Signed bottom heights govern submerged steps; surface movement uses zero for
collision checks. Surface ice has no elevation cost based on its water depth.
Ground motion does not call the upward-breakout action merely because bottom
selection changes altitude: the reference checks breakout before this selection.

Falls beneath intact ice now retain the bottom and skip surface fracture dice.
Standing is allowed there with ordinary control and recovery timers. Shared water
entry checks and flooding apply below the ice. Tests cover surface holds and
breaks, neighbors, zero-depth ice, both movement directions, underwater routes,
depth-one surfacing, falls and standing, saved replay, and complete rollback when
a neighboring fall cannot be resolved.
Read-only references: `movement/mech_motion_integration.c`,
`movement/mech_update.c`, `movement/mech_update_hex.c`,
`movement/mech_update_hex_mech.c`, `movement/mech_landing.c`, and
`movement/mech_ice.c`.


### Airborne ice crossings and interrupted landing

Vertical flight samples now check the previous hex for crossings of elevation -1.
Downward crossing breaks that ice, drops surface neighbors and applies the
triggering fall. The jump then completes its horizontal entry through the shared
ground-segment resolver, without a second acceleration tick or surface selection.
This preserves the fall's altitude when the final hex has a different bottom.
Depth-zero ice breaks without forcing a fall.

Upward crossing drops neighbors but excludes the breaking unit. Its flight and
dice continue unchanged. A completed jump already in its destination hex lands
before the vertical crossing check: a landing fracture drops the unit normally;
if ice holds after a submerged arrival, upward breakout excludes the now-landed
unit and preserves its surface altitude over the replacement water.

Tests cover downward crossing on the final horizontal step, neighboring falls,
complete rollback after a neighbor failure, upward crossing from a saved flight
with changed conditions, both destination-landing outcomes, dice consumption and
restart replay. The saved-condition fixture represents ice over an already
submerged flight; it does not add an ice-growth command. Launch below elevation
-1 remains rejected by the reference's independent takeoff rule.
Read-only references: `movement/mech_motion_integration.c`,
`movement/mech_update.c`, `movement/mech_ice.c`, and `movement/mech_jump.c`.


### High-water movement and running controls

High-water terrain supports ground travel and jump routes at its mapped positive
height. Ground entry uses a -2 terrain piloting modifier at every mapped height,
including zero. Running entry adds the usual +2 modifier and caps forward
throttle at walking speed. Failed checks use wet fall damage. High water does not
count as ordinary immersion for section flooding or extra cooling.

New running-speed commands are rejected in high water or while below elevation
zero in water, ice or bridge terrain. Surface ice and bridge decks retain normal
speed controls. Native commands and Lua share the restriction, including the
walking-speed tolerance; rejection preserves the previous throttle. Existing
running motion crossing a shoreline still reaches the entry check and throttle
cap.

Tests cover high-water entry success and failure, exact dice consumption, wet
fall damage, mapped jump landing heights, native/Lua parity, throttle boundaries
and save/reload replay. Read-only references: `movement/mech_update_hex_mech.c`,
`movement/mech_move_controls.c`, `movement/mech_falls.c`,
`movement/mech_landing.c`, and `map/map_terrain.c`.


### Building, wall and overlay traversal

Biped ground routes now accept every decoded terrain identity. Buildings and
walls use mapped elevation for step costs, reverse checks and cliffs; their
symbols alone do not stop a unit. Live fire and smoke tiles permit movement,
with environmental heat and visibility handled by their existing systems.
Jump routes also accept buildings and walls. Height determines an obstacle
collision or landing surface through the same resolver as ordinary hills.

Tests extend forward/reverse slope and cliff scenarios to both structures,
including falls, rollback and saved replay. Native/Lua jump controls cover clear
crossings and tall obstructions; uniform mapped-height landings cover heights
zero, three and nine. Live overlay crossings preserve their terrain and consume
no control dice. Building interiors, structural damage/collapse, map links and
mines remain separate unfinished systems.
Read-only references: `movement/mech_collision.c`, `movement/mech_update_hex.c`,
`movement/mech_update_hex_mech.c`, `movement/mech_update_speed.c`, and
`movement/mech_landing.c`.


### Crowded-hex collision resolver

The shared Rust stacking resolver owns team-based occupancy, target selection,
relative ground velocity, jump/fall impact energy, configured damage scaling,
physical hit tables, piloting avoidance and falls. It commits both affected units
and their dice together. More than two friendly bipeds trigger friendly selection;
otherwise more than six total bipeds trigger hostile selection. Shut-down units
count toward occupancy but cannot be selected as targets. The selection cursor's
first-candidate weighting is retained, with stable object-ID ordering.

Damage arrives in groups of five. Ground collisions use normal hit locations;
landing collisions use punch locations on the target and kick locations on the
moving unit, with normal locations for prone recipients. Avoidance mode stops
ground motion even after a successful check. Each cockpit uses its own pilot's
toughness setting. Tests cover threshold no-ops, team priority, target-selection
gaps, avoidance success/failure, physical tables, relative velocity, mass rounding,
damage compression, persistence replay and rollback after the first recipient has
already taken damage.

Ground-entry and landing hooks are now connected; explicit fall hooks remain pending. Collision damage
requires calculated current mass, not nominal template tonnage: remaining armor,
internal structure, ammunition, equipment and construction technology affect it.
`BattleStackingInput` therefore requires explicit mass in 1/1024-ton units and
adjusted jump movement points. The current-mass calculator now supplies ground-entry collisions. Landing callers supply adjusted jump capacity while preserving event ordering;
explicit fall callers remain pending. No gameplay
adapter substitutes nominal tonnage or exposes arbitrary player-triggered damage.
Read-only references: `movement/mech_domino.c`, `unit/mech_consistency.c`,
`unit/template_save.c`, and `combat/mech_hitloc_targeting.c`.


### Conventional current mass and automatic ground crowding

`BattleUnit::mass` derives a component breakdown in 1/1024-ton units from the owned
standard-fusion definition and current damage. Engine rating uses original speed,
not damage-reduced mobility. Remaining structure and armor use the reference's
half-ton rounding, including its one-unit tolerance. Weapon slots retain physical
mass after critical destruction until their section is lost; ammunition mass uses
remaining rounds. Heat-sink losses affect the separately counted sink mass.
Destroyed center torsos preserve the signed gyro accounting in the reference.

The current equipment catalog includes exact weapon mass and per-slot rounding;
this makes an intact AS7-D six fixed-point units lighter than nominal tonnage.
Unsupported construction technologies remain rejected by unit construction.
Mass is derived, so persistence needs no new table or cached mutable value.
Wizard inspection displays current tons and Lua `btech.unit.state(id).mass`
returns a detached component breakdown.

Completed ground hex entries now invoke stacking using current mass and configured
stacking mode, damage percentage and hit arcs. Movement, both units' damage, dice
and notices share the tick transaction. Tests cover standard Jenner/Atlas totals,
armor/ammunition/section/sink changes, damaged-slot retention, native/Lua inspection,
disabled/avoidance/damage ground-entry modes, restart replay and rollback after
recipient damage. Jump-obstacle and airborne-shutdown fall crowding is connected; airborne critical-damage crowding is also connected; other lifecycle fall hooks remain pending.
Read-only references: `unit/mech_consistency.c`, `unit/mech_identity.c`,
`unit/mech_specification_state.c`, `unit/weapons_catalogue.c`, and
`movement/mech_update.c`.


### Automatic landing crowding

Normal jump completion and successful early-landing commands now share configured
crowding resolution after ice and balance checks, before final flooding and
stabilization. Failed or prone landings skip the jump-crowding check. The arriving
unit is excluded from occupancy at this stage, matching the airborne reference
ordering: three friendly ground occupants can trigger a collision, while two do
not. Ground and fall entry modes continue to count a grounded moving unit.

Landing collision energy uses remaining jump jets and current mass. Its gravity
adjustment applies only when special map conditions are enabled, independently
of the flight curve's gravity rules. Native and Lua early landings use the same
stacking mode, damage scale, hit arcs and fall rules as the server jump heartbeat.
Stacking policy is carried by the shared damage rules so nested airborne critical
falls use the same configuration as their enclosing action.

Tests cover occupancy boundaries, disabled/damage/avoidance modes, successful and
failed avoidance, two-unit rollback, post-landing persistence, native/Lua parity,
callback rollback, and special-condition gravity damage. An existing ice geometry
fixture now selects safe fall dice so unrelated critical cascades cannot invalidate
its stabilization assertion. Further lifecycle fall collision call sites remain pending.
Read-only references: `movement/mech_landing.c`, `movement/mech_domino.c`, and
`unit/mech_runtime_state.c`.


### Jump-obstacle and airborne-shutdown fall collisions

Failed jump-obstacle landings now resolve crowding after fall damage at the
rolled-back position. Airborne thermal shutdowns also resolve crowding after
the fall and before clearing power, pilot assignment and stabilization. These
are explicit callers: ordinary failed landings, water slips and ground thermal
balance falls do not acquire an unconditional collision trigger.

Thermal airborne fall severity now uses surviving jump movement points, with
gravity adjustment only under special map conditions. This corrects the initial
one-level placeholder. The fall uses pre-impact thrust; subsequent collision
energy uses post-impact equipment and mass. Physical input calculation is shared
with normal landings and keeps physical thrust separate from permission to launch.

Tests cover post-fall collision ordering, a five-level/20-point Jenner shutdown
fall, disabled/damage stacking modes, rollback after a neighboring impact fails,
whole-state replay and persistence after shutdown. Other shutdown and environment fall callers remain pending.
Read-only references: `movement/mech_update_hex_mech.c`,
`movement/mech_overheat.c`, and `movement/mech_domino.c`.


### Airborne critical falls inside damage transactions

Airborne gyro and last-jet falls now resolve crowding at the critical event, after
the fall and before subsequent criticals or damage groups. Current mass and thrust
are sampled after damage. Last-jet loss therefore consumes the eligible target
selection but deals no collision damage when no thrust remains. A failed neighbor
impact restores the complete enclosing damage transaction, including flight,
criticals, both units and their dice.

Stacking policy now travels with `BattleFallRules`, which already supplies nested
tactical damage and fall behavior. Shot adapters forward their configured policy;
movement and jump completion share the same field. The separate jump-rules wrapper
and duplicate movement stacking field were removed. This keeps one policy value
through each transaction rather than introducing a callback or global setting.

Tests exercise configured gyro collisions, last-jet falls without neighbor damage,
disabled stacking, rollback and saved replay. Existing native/Lua firing, landing,
movement and thermal tests exercise the updated rule plumbing. Other shutdown and
environment lifecycle fall triggers remain pending.
Read-only references: `combat/crit_mechs.c` and `movement/mech_domino.c`.


### Ground shutdown while moving

Voluntary ground shutdown now permits movement. Forward speed strictly greater
than 10.75 kph causes a one-level fall and then the explicit fall-crowding check;
reverse movement and speeds at or below the threshold stop without that fall.
The running unit centers its torso before shutdown. Safe shutdown preserves arm
orientation; a fall uses the existing full facing reset. Power, pilot assignment,
stand timer, target lock and motion are cleared after the effects resolve.

Native and Lua shutdown share configured damage/stacking rules and an atomic
candidate. They publish all notices only with the enclosing command commit.
Tests cover the exact positive-speed boundary, reverse travel, facing, both
adapters, callback rollback, failed neighbor damage, disabled collisions and saved
state. The domain stop operation now returns ordered notices and accepts damage
rules explicitly. Airborne voluntary shutdown schedules free fall; startup abort
continues through the existing path.
Read-only references: `movement/mech_startup.c` and `unit/mech_lifecycle.c`.

### Airborne shutdown and free fall

`BattleFreeFall` models BattleMech vertical descent in committed seconds. It stores
integer altitude, drop speed and the remaining one-to-three-second countdown.
The first event drops two levels, subsequent events accelerate by one level, and
contact occurs when the distance to the caller-selected surface is no greater
than the event's drop speed. Impact severity is `speed * (speed + 1) / 2`, even
when the remaining distance is shorter. Map gravity and movement percentages do
not scale this event. Restarting a BattleMech engine does not arrest its fall.

Airborne shutdown replaces the jump trajectory with this cursor, stops horizontal
travel, releases the pilot and turns off the engine. The one-second simulation
advances the cursor even while powered off. Contact uses the ice top or, when
below a bridge deck, its lower surface at elevation -1. The cursor preserves the
pre-contact altitude until the world transaction applies fall damage, flooding
and ice effects and removes it. This impact does not run a crowding check.
Serialization rejects invalid countdowns and zero speed. Tests replay every
saved second, cover exact contact and changed surface heights, and check atomic
arithmetic failure. Native/Lua shutdown parity, callback rollback, database replay
at each second, impact failure/retry, engine restart during descent and ground,
water, ice and bridge contact have integration tests. Lua inspection exposes
`free_fall`; range and LOS retain its altitude. Administrative placement/removal
and destruction cancel the descent. Aircraft recovery remains unsupported.
After the engine restarts, `land` can attempt an early landing during free fall.
Success or failure ends airborne movement but retains the pending fall event,
including its speed and countdown. The scheduled event still causes an impact
when due. A grounded pending event does not contribute airborne height to range
or LOS; Lua marks it with `grounded = true`. Tests cover both landing outcomes,
callback rollback and persisted replay through the retained impact. Stabilization
counts down while powered off and only announces completion when running.
A live TCP server test rejects shutdown and impact saves, verifies that rejected
notices do not reach the cockpit, restarts with the pending event, and checks
that retry produces the expected damage and dice state.
Read-only references: `unit/mech_events.c::mech_fall_event`,
`movement/mech_landing.c::mech_height_above_surface`, and `core/btconfig.h`.


### Conventional energy catalog expansion

Small lasers, large lasers and PPCs now use the shared loadout, aim, direct-fire,
heat, damage and recycle pipeline. Their heat/damage/recycle values are 1/3/15,
8/8/25 and 10/10/30 respectively. A PPC occupies three criticals and has minimum
range three; its short/medium/long brackets are 6/12/18. Small laser mass is half
a ton, large laser mass is five tons, and PPC mass is seven tons, before current
mass per-critical rounding.

The unchanged game AWS-8Q and HBK-4P assets resolve and construct with these
weapons. Explicit zero `Cargo_Space` and `Max_Suits` values mean no carrying capacity;
nonzero or invalid capacity is still rejected. Descriptive comments are retained. Tests cover those assets, catalog and
range boundaries, native/Lua shots, callback rollback, single-hit damage, heat,
recycle notices and saved replay. Underwater fire and special energy modes remain
unsupported. References: `unit/weapons_catalogue.c`, `unit/weapons_vrt.h` and
`unit/mech_identity.c` in the read-only reference tree.


### Conventional autocannon catalog expansion

AC/2, AC/5 and AC/10 now share the existing direct-fire, ammunition, heat, critical
and recycle rules. Their heat/damage/recycle values are 1/2/12, 1/5/20 and 3/10/25;
full bins contain 45, 20 and 10 rounds. Minimum ranges are four, three and zero,
with short/medium/long brackets of 8/16/24, 6/12/18 and 5/10/15. Their masses are
six, eight and twelve tons, before per-critical rounding, and extended gunnery
uses `Gunnery-Ballistic`.

The unchanged ENF-4R game asset now constructs with its AC/10, large laser and
small laser. Tests cover all three added autocannons' catalog/range boundaries,
bin limits, explosion potential, mass and ammunition use. The shared native/Lua
fire test includes rollback, single-group damage, heat, recycle and restart replay.
Rapid-fire modes, special ammunition, jams and underwater fire remain unsupported.
References: `unit/weapons_catalogue.c` and `unit/weapons_vrt.h` in the read-only
reference tree.


### Conventional missile launcher sizes

SRM-2 and LRM-5/10/15 now use the shared missile-hit and direct-fire rules.
SRM-2 consumes one of 50 salvos per full bin, generates two heat, and recycles in
15 seconds. LRM-5/10/15 capacities are 24/12/8 salvos, heat is 2/4/5, and recycle
times are 15/20/25 seconds. SRMs resolve each missile as a separate two-damage hit;
all LRM sizes resolve groups of up to five one-damage missiles. Glancing hits
use the same reduced cluster roll across every supported launcher size.

Tests cover all 36 two-dice outcomes, grouped damage, glancing clusters, catalog
facts, bin hazards and mass. Native/Lua shots exercise ammunition, heat, rollback,
recycle and saved replay. Existing GRF-1N, CPLT-C1 and TBT-5N assets now construct
without edits. Indirect fire, guided/streak/swarm behavior and special ammunition
remain unsupported. References: `unit/weapons_catalogue.c`, `unit/weapons_vrt.h`
and the missile-hit registry facts in `unit/mech_build.c` in the read-only tree.


### Machine guns and ordinary flamer fire

Machine guns and flamers now support ordinary direct damage in tactical biped
combat. Both inflict two damage in a single hit at ranges 1/2/3. Machine guns
produce no heat, recycle in seven seconds, weigh half a ton and use bins of 200
rounds. Flamers produce three heat, recycle in ten seconds, weigh one ton and
consume no ammunition. Ordinary flamer fire damages armor rather than adding
heat to the target. Flamer heat-transfer mode is described below; machine-gun
Gattling mode remains rejected; infantry-specific effects remain outside supported unit classes.

The reference classifies the beam flamer as an energy weapon before checking its
name, so extended gunnery uses `Gunnery-Laser`; machine guns use
`Gunnery-Ballistic`. Existing FS9-H and LCT-1V assets now construct unchanged.
The optional trailing dash on an ammunition line represents no brand. `FlipArms`
spelling is case-insensitive in validation and controls, including the Locust's
`Fliparms` spelling. Tests cover these assets, a 400-damage full machine-gun bin,
unsupported mode rejection, native/Lua fire and recycle replay, and arm flipping
across a database restart.

Read-only references: `unit/weapons_catalogue.c`, `unit/weapons_vrt.h`,
`unit/mech_identity.c`, `unit/mech_weapons.c`, `unit/template_load.c`,
`unit/template_format.c`, and `combat/mech_hit_resolution.c`.


### Flamer heat-transfer mode

`flamerheat <selection>` toggles intact, recycled flamers between normal
and heat modes. Selections accept comma-separated numbers and inclusive ranges
(for example `0,2-4`), with reversed ranges visited in ascending order and repeated
numbers toggled again. Use no spaces within the selection. Invalid clauses stop
the selection while preserving earlier changes; unsuitable weapons report an
error to the pilot and processing continues. Successful changes notify the cockpit.
Lua uses `btech.unit.flamerheat(unit, pilot, weapon)` and returns
`normal` or `heat`. Both paths require the conscious assigned pilot and a running
engine, stage the same cockpit notices, and roll back with the enclosing command
or callback on transaction failure. Lua takes one weapon per call.

A successful heat-mode shot adds two stored weapon heat to its target instead of
rolling a damage location or changing armor. Glancing hits still transfer two
heat; misses transfer none. The shooter still pays three heat and ten seconds of
recycle. The ordinary thermal tick later processes the target's added heat.
Shot reports expose `heat_transfer`; `salvo` is absent for these hits as well as
misses. Native output correctly reports a hit and the plasma notices.

Live modes are persisted independently of initial template flags. A template
`Heat` flag is accepted only on flamers and can coexist with `RearMount`; duplicate
or incompatible modes are rejected. Native `weapons` displays `[HEAT]`, while Lua
weapon inspection exposes `fire_mode` and loadouts expose `initial_fire_mode`.
Tests cover template geometry, corrupt saved overrides, mode guards, native/Lua
parity, callback rollback, restart and recycle, hits, glances, misses, and unchanged
target damage dice. Selection tests cover reversed ranges, repeats, partial errors,
bounded indices and notification-failure rollback. Reference: `combat/mech_weapon_modes.c`,
`combat/mech_hit_resolution.c` and `unit/mech_heat_state.c` in the read-only tree.


### Inner Sphere advanced energy weapons

The catalog now includes ER small/medium/large lasers, ER PPCs, small/medium/large
pulse and X-pulse lasers, and light/heavy/Snub-Nosed PPCs. Their heat, damage, ranges,
critical-slot counts, mass and recycle times use their individual catalog values.
ER PPCs have no minimum range; light and heavy PPCs retain the three-hex minimum.
Pulse and X-pulse lasers contribute a signed -2 target-number adjustment, exposed
as `weapon_accuracy` in the shot's aim breakdown. Range penalties, damaged mounts,
heat and sensor contributions remain separate. Glancing direct hits still round
half damage upward.

These weapons use the shared native/Lua firing, expenditure, damage and persistent
recycle paths. Tests cover all thirteen weapons through callback rollback, firing,
save/load and every recycle tick, plus numeric catalog/range boundaries and a
seeded pulse hit-boundary comparison. Unsupported chassis technologies still
prevent construction, and Clan equipment, underwater combat and capacitor modes remain pending. Reference facts:
`unit/weapons_catalogue.c`, `unit/weapons_vrt.h` and `combat/mech_bth.c`.


Snub-Nosed PPC hits deal ten damage through exactly nine spatial hexes, eight
through thirteen, and five beyond thirteen. These thresholds use unrounded
spatial distance, independently of aim-bracket rounding. Glancing damage is
halved afterward and rounded up to five, four or three. Direct fire computes
range from both live units. Range-independent damage APIs reject this weapon
when attack range is unavailable; `damage_groups_at_range` supplies it explicitly
for domain callers. Native/Lua tests fire at exact and fractional thresholds
from saved/reloaded positions. Reference: `combat/mech_hit_resolution.c`.


### Inner Sphere double heat sinks

`DoubleHS` chassis now construct with complete three-slot `HeatSink` groups.
It can coexist with `FlipArms`; chassis flags are whitespace-separated and
case-insensitive. Unknown chassis technology remains rejected. Template
`Heat_Sinks` records cooling capacity, so a value of 24 dissipates 24 heat per
turn before environmental adjustments, rather than being doubled again.

A critical hit destroys all three slots in the affected sink and removes two
cooling capacity. Flooding or losing its section removes the same capacity once
per group. Saved partial-group losses and incomplete installations are rejected.
Mass converts capacity to physical sinks, subtracts the engine's ten free sinks,
and retains the reference's per-critical rounding (three times 341 mass units
for each extra double sink). Water cooling continues to count surviving immersed
slots and apply the existing bonus limits.

The existing BJ-3 and APL-1R assets construct unchanged. Tests cover those assets,
combined flags, missing/split groups, hits on each slot, repeated hits, flooding,
mass, thermal replay after restart, Lua loss inspection and arm flipping. Clan
and compact sink technology, sink disabling and repair remain pending. Reference:
`unit/mech_specification_state.c`, `unit/mech_weapons.c`, `unit/mech_consistency.c`,
`combat/crit_mechs.c` and `combat/environment_damage.c`.


### Distributed armor and structure materials

Inner Sphere Ferro-Fibrous, light/heavy Ferro-Fibrous, and Endo Steel slots are
recognized as construction materials. They occupy critical slots but are excluded
from random critical selection and add no separate equipment mass. Technology is
derived from installed slots, including slots in damaged or lost sections. Flags
alone do not activate it: standard Ferro-Fibrous and Endo Steel require at least
14 slots, light Ferro-Fibrous seven, and heavy Ferro-Fibrous 21. Smaller allocations
remain occupied material slots without a mass bonus.

Armor mass applies the reference's integer conversion before half-ton rounding:
standard Ferro-Fibrous uses 50/56, light 50/53, and heavy 50/62 of ordinary armor
point accounting. Standard Ferro-Fibrous takes precedence over heavy, then light,
if multiple complete allocations coexist. Endo Steel halves the ordinary
structure mass calculation before rounding. Neither technology changes armor or
internal damage-point capacity; current mass still falls with actual damage.

The unchanged CRB-28 asset now constructs, with saved/reloaded mass and Lua
inspection covered. Rule tests cover thresholds, flags without slots, rounding,
armor/internal damage, retained material benefits and critical exclusion. Clan
materials and remaining advanced construction technologies remain pending.
Reference: `unit/template_specials.c`, `unit/mech_consistency.c` and
`combat/crit_dispatch.c`.


### Fusion-engine installations

Canonical Inner Sphere standard, light, XL, XXL and compact fusion layouts are
supported. Engine identity is derived from installed slots, retained after damage,
and exposed as `engine` in Lua unit state. Six center-torso slots and zero/two/three/
six slots in each side torso identify standard/light/XL/XXL engines; three center
slots with no side slots identify compact engines. Complete relocated layouts
are also supported as described below. Incomplete, conflicting and non-torso
installations are rejected. Declared engine flags do not substitute
for the required equipment.

The standard engine catalog is scaled by 3/4 for light, 1/2 for XL, 1/3 for XXL,
and 3/2 for compact engines, then rounded to half tons using the shared reference
rounding rule. Engine slot losses continue to add five heat each while running;
the third hit destroys the unit and releases its pilot. Side-torso loss therefore
destroys canonical XL/XXL units, while a light engine can survive one lost side torso with
two engine hits. Engine type and mass accounting remain derived from construction
through shutdown, loss and restart.

Tests cover all five types, rounding, incomplete-layout rejection, torso loss, individual
hits, native/Lua shutdown parity and saved engine inspection. The unchanged AF1
Arctic Fox asset now constructs with its inferred XL engine. Clan, large-engine
slot extensions, combustion engines, reactor explosions and repair remain pending.
Reference: `unit/template_specials.c`, `unit/mech_consistency.c`,
`combat/crit_mechs.c` and `combat/environment_damage.c`.


### CASE ammunition containment

Installed Inner Sphere `CASE` (including the existing `Case` spelling) prevents
internal explosion damage from leaving its section. The section and attached arm
can still be destroyed, ammunition is spent, and explosion injury still applies.
CASE does not absorb ordinary weapon damage, including the triggering shot's
remaining damage after a nested explosion. XL/XXL side-torso engine loss can still
destroy the unit even when the center torso is protected.

CASE occupies a slot, adds half a ton while its section remains, and is excluded
from random critical selection. Containment is derived from installed equipment,
including in a section just destroyed by the explosion; a section `Config { Case }`
annotation alone does not install CASE. Tests compare protected/unprotected
explosions, local and engine destruction, later weapon transfer, empty-bin guards,
restart replay, native/Lua firing and callback rollback. The unchanged HBK-5M
asset constructs with its mixed-case part spelling. Clan integral containment,
CASE-II and repair remain pending. Reference: `unit/template_specials.c`,
`unit/mech_consistency.c`, `combat/crit_dispatch.c` and `combat/mech_damage.c`.


### Streak SRM launchers

Inner Sphere Streak SRM-2/4/6 launchers now use the shared firing pipeline. A lock
requires the ordinary target number, even with the below-target glancing setting.
A failed or out-of-range lock consumes the shooter roll and starts a fifteen-second
recycle, but spends no ammunition or heat and rolls no target damage. Shot reports
expose `launched = false`, zero expenditure heat, and no expenditure ammunition bin
for that outcome. Only the firing cockpit receives the failed-lock notice.

Successful locks launch every missile as separate two-damage groups. Glancing
rules do not reduce these salvos or produce glancing notices. The shared cluster
roll is retained in the report and replay stream, while its value does not reduce
the missile count. Each rack's heat, ammunition capacity, mass and critical slots
use the catalog values. The unchanged BJ-2 asset now constructs.

Tests cover all rack sizes through native/Lua firing and recycle replay, failed
lock resource use, boundary rules, out-of-range attempts, callback rollback and
saved failed-lock timers. Angel ECM interference, special ammunition and Clan
launchers remain pending. Reference: `combat/mech_fire_resolution.c`,
`combat/mech_combat_missile.c`, `unit/weapons_catalogue.c` and `unit/weapons_vrt.h`.
CASE-II remains unsupported: the reference has catalog definitions but no CASE-II
combat handling in the inspected tree.

### Medium-range missiles

Inner Sphere MRM-10/20/30/40 launchers use the shared firing pipeline, with a +1
accuracy modifier and short/medium/long ranges of 3/8/15 hexes. Each missile deals
one damage, grouped into independent hits of up to five damage. Each rack uses
its own cluster table; MRM-10 differs from LRM-10, and MRM-30 is not a scaled
LRM table. Glancing hits shift the cluster roll down four, with one missile
when the shifted roll falls below two.

Catalog tests cover heat, critical slots, mass, ammunition capacity, range
boundaries and recycle times. All 36 two-dice outcomes and glancing tables are
checked, and all four racks pass native/Lua firing, callback rollback and saved
recycle replay. The existing QKD-8K asset constructs unchanged. Reference facts:
`unit/weapons_catalogue.c`, `unit/weapons_vrt.h`, `unit/mech_build.c`,
`unit/mech_weapons.c` and `combat/mech_bth.c`.

### Extended-range LRMs

Inner Sphere ELRM-5/10/15/20 launchers share missile firing, with 12/24/36 ranges,
a ten-hex minimum and thirty-second recycle. Below exactly ten spatial hexes,
the attack roll uses the lowest two of three dice; at ten hexes and beyond it
uses ordinary two-dice rolls. This selection uses unrounded distance and applies
in addition to the normal minimum-range aim penalty. The reported attack result
and saved shooter stream include the additional die.

Cluster rolls remain ordinary two-dice rolls, with ELRM-specific hit tables and
five-point damage groups. ELRM-10 and ELRM-15 differ from their conventional LRM
counterparts. Glancing hits use the shared four-point cluster-roll reduction.
Tests cover catalog facts, minimum/maximum and fractional range boundaries,
seeded attack dice and replay, all cluster outcomes, glancing tables, native/Lua
fire, callback rollback and persisted recycle. Indirect fire and additional
missile modes remain pending. Reference facts: `unit/weapons_catalogue.c`,
`unit/weapons_vrt.h`, `unit/mech_build.c` and `combat/mech_fire_preparation.c`.

### Dead-fire missiles

IS.LR_DFM-5/10/15/20 and IS.SR_DFM-2/4/6 use the shared firing pipeline. Their
attack rolls always use the lowest two of three dice, regardless of range.
Long-range launchers have 6/12/18 ranges and a four-hex minimum; short-range
launchers have 2/4/6 ranges with no minimum. A hit rolls the ordinary cluster
check, with the same hit counts as corresponding ELRM and SRM racks respectively.

Every missile gets its own hit location: two damage for LR dead-fire, three for
SR dead-fire. LR dead-fire does not use the five-missile grouping of ordinary
LRMs. Glancing hits reduce cluster rolls by four without halving each missile's
damage. Ammunition explosions use the increased per-missile damage.

Coverage includes catalog values, individual packets, all cluster outcomes and
glancing tables, attack dice across ranges, ammunition capacity and bin hazards,
native/Lua firing, callback rollback and saved recycle replay. Indirect fire and
additional missile modes remain pending. Reference facts: `unit/weapons_catalogue.c`,
`unit/weapons_vrt.h`, `unit/mech_build.c`, `unit/mech_weapons.c` and
`combat/mech_fire_preparation.c`.

### Gauss weapons and inert ammunition

IS.GaussRifle, IS.LightGaussRifle and IS.MagshotGaussRifle share direct firing,
ammunition expenditure, heat and recycle handling. Their projectile damage is
15/8/2 and recycle time is 30/20/12 seconds respectively. Catalog slots, mass,
ammunition capacities and ranges are represented individually. The existing
HGN-732 asset constructs unchanged.

Gauss ammunition contributes zero explosion potential. Heat hazards skip these
bins; critical hits disable the feed without explosion or explosion injury.
The first critical on a functional weapon instead destroys every mounting slot
and reports 20/16/3 internal explosion damage for standard/light/Magshot rifles.
The enclosing damage transaction resolves transfer, CASE containment, nested
criticals and crew injury. All slots are disabled before the cascade, preventing
repeat explosions. Ordinary weapon criticals still disable individual slots.
`BattleCriticalLoss::Weapon` includes `explosion_damage` for this composition.

Tests cover each initial mounting-slot hit, depleted critical selection, inert
bins, all three explosion sizes, internal damage bypassing armor, CASE transfer,
seeded replay and restart. Native/Lua firing covers resource use and recycle;
weapon-triggered explosions also cover cockpit notices, callback rollback and
saved state. Clan weapons and weapon repair remain pending. Heavy Gauss behavior is described below. Reference facts: `unit/weapons_catalogue.c`,
`unit/weapons_vrt.h`, `combat/crit_weapons.c`, `combat/crit_mechs.c` and
`combat/mech_damage.c`.

### Heavy Gauss damage and recoil

IS.HeavyGaussRifle now supports an eleven-slot mount within one section, eighteen
tons of catalog mass, four inert shots per ammunition bin, two heat per shot and
thirty-second recycle. Minimum range is four, with 6/13/20 range brackets. Raw
spatial range determines damage: 25 through six, 20 through thirteen and 10
beyond thirteen, before glancing damage is halved and rounded up. Range-free
damage requests are rejected. Weapon criticals destroy the complete mount and
release 25 internal damage through the shared Gauss explosion path.

Moving shooters roll a piloting check after the target's damage resolves, including
on misses. The modifier uses nominal chassis tonnage: +2 through 35 tons, +1
through 55, zero through 75, and -1 above 75. Forward and reverse movement both
qualify; stationary firing consumes no recoil dice. Failed checks apply a
one-level fall, including ordinary protection, damage, posture and terrain effects.
Shot reports expose the check and optional fall in `recoil`. The shot, recoil,
notices and all affected unit state share one transaction.

Tests cover raw damage thresholds, glancing values, all weight-class boundaries,
stationary/forward/reverse shots, hits and misses, control success/failure,
native/Lua firing, callback rollback and persisted replay. Linked split critical
mounts are also supported. CES-4S constructs with its relocated light-engine layout.
Reference facts: `unit/weapons_catalogue.c`,
`combat/mech_hit_resolution.c` and `combat/mech_fire_resolution.c`.

### Relocated fusion-engine slots

Complete Inner Sphere light, XL and XXL installations can move side-torso engine
slots into the center torso. The installed side counts still identify the engine
family, independent of declared technology flags. The supported complete layouts
have six to eight center slots and retain the family's total: ten for light,
twelve for XL, eighteen for XXL. Nonempty side counts must be two for light,
one or three for XL, or four through six for XXL. Incomplete totals, conflicting
side evidence and non-torso engine slots remain rejected.

Engine type and mass remain fixed by the original installation after damage.
Heat and the three-hit destruction rule follow actual lost slots. Consequently,
a relocated XL engine can survive loss of its one-slot side torso, and a relocated
light engine takes no engine damage from loss of its empty side torso. Destroying
relocated center-torso slots counts like any other engine critical.

The existing CES-4S uses eight center slots, two left slots and no right slots;
it now constructs as a light-engine biped and fires its Heavy Gauss unchanged.
Tests cover every supported relocated count pattern and mirror, mass rounding,
incomplete/conflicting layouts, slot damage and retained identity. The Cestus
scenario covers native/Lua fire, callback rollback, left/right torso loss, engine
heat, the third-hit kill and restart replay. Reference facts:
`unit/template_specials.c`, `unit/mech_consistency.c` and `combat/crit_mechs.c`.

### Split weapon mounts

AC/20, LB/20-X and Heavy Gauss mounts can extend beyond the primary section using explicit
`SplitCrit_Left` or `SplitCrit_Right` template markers. Each marker's data is the
zero-based first weapon slot in the linked primary section. The primary weapon
run fills its section through slot twelve; exactly the remaining catalog slots
must form one contiguous extension run. The supported links join an arm to its
same-side torso, or a side torso to its arm, leg or the center torso. Marker
handedness, parent existence, slot counts and metadata are validated. Orphaned,
misdirected, incomplete and inconsistent links are rejected.

The resolved mount stores primary slots first and extension slots afterward.
It has one weapon number, ammunition selection, fire mode and recycle timer.
Primary slots determine firing arcs and mounting modifiers. Every slot contributes
to readiness, flooding and physical mass; a lost section removes only its own
weapon-slot mass. Markers are resolved links, not independent systems.

An extension critical belongs to the same weapon. Gauss destruction disables
all primary and extension slots before resolving its explosion in the primary
section; CASE in that section contains transfer. Tests cover every supported
link direction, malformed links, ordinary and Gauss critical availability,
section mass loss, primary-section explosion origin and CASE, saved replay,
native/Lua fire, callback rollback and recycle persistence. Enhanced weapon
critical modes remain a separate unfinished rule family. Reference facts:
`unit/mech_ammunition.c`, `combat/crit_mechs.c`, `combat/mech_section_damage.c`
and `unit/mech_consistency.c`.


### LB-X autocannons and ammunition selection

IS.LB2-XAC, IS.LB5-XAC, IS.LB10-XAC and IS.LB20-XAC support ordinary slugs
and `LBX/Cluster` ammunition. Mounts and bins retain separate typed ammunition
modes. Empty flags mean normal ammunition; `LBX/Cluster` is accepted only for
LB-X equipment. The initial mount mode comes from its template flags; later
changes persist independently of the template. LB/20-X also supports explicit
split critical links.

`lbx <selection>` and `btech.unit.lbx(unit, pilot, weapon)` share pilot, startup,
critical and recycle guards. Mode changes require no ammunition and participate
in command/Lua rollback. `weapons` displays `[LBX]` in cluster mode, and the Lua
inspection exposes `ammunition_mode`. Bin availability and expenditure match
both weapon type and selected ammunition mode, preferring the primary mounting
section. They never fall back to the other ammunition type.

Slugs use ordinary direct damage. Cluster mode adds −1 to the attack modifier,
uses the size-specific 2d6 cluster table, and resolves each pellet as a separate
one-point hit. Glancing fire shifts the cluster roll down four, with one pellet
below the table. Both ammunition types retain the same nominal explosion
potential. Heat, recycle and ammunition expenditure remain shared with ordinary
firing. Shot reports record the ammunition mode actually used.

Tests cover all catalog profiles, all 36 cluster outcomes, glancing tables,
unchanged UM-R63 construction, matching-bin exhaustion, native/Lua mode and fire
parity, callback rollback and persisted recycle replay. Reference facts:
`unit/weapons_catalogue.c`, `unit/mech_build.c`, `unit/template_flags.c`,
`combat/mech_bth.c`, `combat/mech_hit_resolution.c` and
`combat/mech_weapon_modes.c`. Other special ammunition remains pending.

### Rocket launchers and self-contained salvos

IS.RL-10/15/20 use their catalog heat, ranges, mass and thirty-second recycle,
with a +1 accuracy modifier and the corresponding LRM cluster table. Each
successful hit resolves in groups of up to five one-point rockets. Glancing
hits shift the cluster roll down four through the shared missile resolver.

The `OneShot` mount flag supplies one self-contained salvo for supported missile
launchers, including rockets. It never draws ammunition from another launcher
or a matching external bin. `OneShot_Used` additionally marks a template mount
as already spent; it requires `OneShot`. Unsupported non-missile uses, duplicate
flags and inconsistent multi-slot metadata are rejected. Rocket construction
requires the explicit `OneShot` flag used by the game's existing templates.
The reference catalog combines ROCKET with IDF, while its special rocket supply
branch tests exact equality; these templates therefore use the ordinary
OneShot supply path.

A launched hit or miss persistently consumes the mount's salvo. A failed Streak
lock starts recycling but preserves its salvo and generates no heat. Critical
damage, shutdown and completed recycle never replenish a spent launcher.
Native `fire` and Lua `btech.unit.fire` share this transaction; callback rollback
restores the salvo, heat, dice, damage and notices. `weapons` labels the mount
`[OS]`, reports its available salvo count and displays `spent` after use. Lua
inspection provides `one_shot` and `readiness.spent`; loadouts also expose
`initially_spent` separately from live state.

Tests cover rocket catalogs, all cluster outcomes, glancing tables, initial
spent templates, independent paired launchers, hits and misses, failed Streak
locks followed by successful launches, unchanged external bins, native/Lua
parity, callback rollback and saved recycle completion without replenishment.
The existing COM-4H constructs unchanged with six independent RL-15 mounts.
Repair/rearming operations remain pending. Reference facts:
`unit/weapons_catalogue.c`, `unit/weapons_vrt.h`, `unit/mech_build.c`,
`unit/template_flags.c`, `unit/mech_systems.c`, `combat/mech_bth.c` and
`combat/mech_ammunition_decrement.c`.

### Light autocannons and the Inner Sphere heavy machine gun

IS.LightAC/2, IS.LightAC/5 and IS.HeavyMachineGun support ordinary direct fire
through the shared native/Lua path. Light AC/2 uses one heat, two damage,
6/12/18 ranges, one critical, four tons, 45 rounds per bin and a twelve-second
recycle. Light AC/5 uses one heat, five damage, 5/10/15 ranges, two criticals,
five tons, 20 rounds per bin and a twenty-second recycle. Neither has a minimum
range penalty.

The reference Inner Sphere heavy machine gun uses zero heat, two damage,
2/4/6 ranges, one critical, one ton, 100 rounds per bin and a seven-second
recycle. This follows the game catalog's values. Ordinary machine-gun or
standard autocannon bins cannot supply these distinct weapon types.

These weapons use ballistic gunnery, ordinary glancing damage, matching-bin
expenditure, ammunition explosions and critical disablement. Tests cover exact
catalog and range boundaries, inert weapon criticals, explosive ammunition,
retained mass after slot damage, bin incompatibility, native/Lua firing parity,
callback rollback and persisted recycle replay. Rapid-fire and gatling modes
remain unsupported and their template flags are rejected. Reference facts:
`unit/weapons_catalogue.c`, `unit/weapons_vrt.h` and `unit/mech_weapons.c`.

### Maintaining the weapon catalog

`src/btech/equipment/catalogue.rs` is the single source for supported weapon
identities, asset names, profiles, mass, gunnery family and recycle feedback.
Its declarative table generates the typed enum and exhaustive accessors at
compile time. `BattleWeapon::ALL` enumerates supported identities for inspection
and coverage checks. Parsing remains strict: adding an asset to the game
directory does not make an unimplemented weapon operational.

Equipment-specific combat rules remain in the domain modules: attack dice and
intrinsic accuracy in `equipment.rs`, ammunition selection in `readiness.rs`,
cluster/damage resolution in `salvo.rs`, and shot transactions in `shot.rs`.
A new weapon needs a catalog entry plus its actual supported behavior and
validation; no changes to generic recycle dispatch or skill selection are needed.

The catalog consolidation is checked against a captured public-API snapshot
of all 62 previously supported weapons in
`tests/fixtures/btech/weapon-catalog.json`. The snapshot includes serialized
identity, asset spelling, mass, full profile, skill and intrinsic accuracy.
Identity uniqueness, strict parsing, recycle labels and existing gameplay,
Lua and persisted replay tests provide additional verification. Intentional
catalog changes should update the corresponding fixture and behavioral tests.

### Installed targeting computers

`TargetingComputer` slots automatically assist eligible direct weapons. Lasers,
including pulse lasers, PPCs, autocannons and Gauss weapons receive a −1 aim
modifier while the shared computer is operational. Flamers, machine guns and
missile launchers receive no bonus. LB-X slug rounds qualify; cluster rounds do
not receive the computer bonus in addition to their ammunition modifier.
Supported weapons may retain the template `OnTC` flag, but assistance requires
installed computer slots. No duplicate live enable flag is stored.

The computer is operational only when at least one slot is installed and none
of its slots is destroyed, in a lost section or flooded. One critical therefore
disables assistance globally even when the installation spans sections. Combat
critical resolution announces the first computer failure. Section destruction
and attack-induced flooding also announce the loss at the corresponding damage
event, without repeating a notice if an earlier critical already disabled the
computer. Slots each contribute
one ton to equipment mass; critical damage retains this mass until the section
is destroyed. The existing damage and flooding records preserve availability
across restart. The live aim report exposes `targeting_computer` independently
of weapon and ammunition modifiers through both Rust and Lua firing.

Tests cover equipment eligibility, stacked pulse assistance, LB-X exclusion,
slot damage, flooding, mass retention and section loss, first-failure combat
feedback through criticals, section destruction and attack-induced flooding, native/Lua firing and rollback, and persisted replay. BL12-KNT
constructs unchanged with its six-slot targeting computer and explicitly linked
energy weapons. Aimed-section attacks and the separate short/long/multi/anti-air
computer configuration modes remain pending. Reference facts:
`unit/mech_weapons.c`, `unit/template_specials.c`, `unit/mech_consistency.c`,
`combat/mech_bth.c`, `combat/crit_mechs.c` and `combat/environment_damage.c`.

### Hardened gyros

The `HDGYRO` chassis flag selects a four-slot hardened gyro. Its mass is twice
the standard gyro mass, using the same engine-rating rounding. Physical critical
losses remain separate from effective damage: the first lost gyro slot absorbs
one hit without a mobility or piloting penalty. The second gives the ordinary
+3 piloting modifier and triggers the first-damage balance check. The third
removes gyro support and forces the ordinary destroyed-gyro fall. Further hits
do not restore support or repeat the initial protection.

Movement, standing and jump landing all use the same derived effective damage.
A protected first hit while airborne does not initiate a fall check or change
the flight. Subsequent damage uses the existing gravity-aware airborne fall
rules. Cockpit notices distinguish the protected hit, damaged gyro, destroyed
gyro and further damage. Construction identity survives critical damage and
restart; no separate damage flag or countdown is stored.

Rust exposes `BattleGyro`, `BattleUnit::gyro()` and `gyro_damage()`. Lua unit
state exposes `gyro` (`standard`, `hardened`, `xl` or `compact`) and `gyro_damage`; existing
mass and mobility inspection reflects the corresponding effects. Tests cover
slot-order independence, mass, all damage thresholds, incomplete installations,
combat checks, flight protection, notices and persisted replay.

`XLGYRO` selects a six-slot gyro with half the standard mass; `CGYRO` selects
a two-slot gyro with 1.5 times the standard mass. Both use ordinary gyro damage:
one hit adds the piloting penalty, and two remove support. Mass scales after
the standard engine-rating rounding and retains the reference destroyed-center
accounting adjustment. Construction rejects incomplete installations and
conflicting gyro technology flags. Tests exercise every critical slot as the
first hit, all subsequent losses, mass, and serialized restoration.

Reference facts: `unit/template_flags.c`,
`unit/mech_consistency.c` and `combat/crit_mechs.c`.

### Explicit half-ton ammunition bins

The `Halfton` bin flag sets installed capacity to half the weapon's ordinary
salvos per ton, rounded down. It is independent of ammunition type and may be
combined with supported `LBX/Cluster` rounds in either flag order. Duplicate or
unsupported flags and ammunition for non-ammunition weapons are rejected.
Raw loadout validation rejects overfilled bins; construction normalizes their
initial quantities as described below. Loadout inspection exposes `capacity` and `half_ton` alongside
initial `rounds` and ammunition `mode`.

Live state is validated against the installed bin capacity. Firing, critical
losses, flooding, ammunition hazards and mass use the existing shared paths;
remaining ammunition mass is rounds times 1024 divided by the weapon's ordinary
salvos per ton, preserving integer rounding for odd-capacity bins. The bin size
and selected ammunition mode survive restart without refilling the bin.

Tests cover every supported ammunition weapon, odd capacities, empty and full
half bins, critical hazards, mode combinations, native/Lua firing, callback
rollback and saved-state overfill rejection. OSR-3D and RZK-9S construct unchanged.
Construction also infers half-ton size when an unmarked initial quantity is
at or below half capacity, then fills the selected capacity. Quantities above
half capacity select a full bin; explicit half bins retain their declared size,
including when the initial quantity is overfilled. Zero and partial quantities
are normalized once in the owned construction definition, leaving the source
asset untouched. Unsupported flags and invalid numeric syntax remain errors.
Live ammunition loading never invokes this normalization and never refills
expended or destroyed bins. Native/Lua construction, boundary values across all
supported ammunition weapons and empty-bin restart behavior are tested.
Reference facts: `unit/mech_weapons.c::full_ammo`,
`unit/mech_consistency.c::ammo_weight` and `unit/template_load.c`.


### Read-only construction checks

`@btech template-check <name>` and `btech.template.check(name)` validate a parsed
asset through the same normalized construction path as unit creation, without
registering an object, changing live dice or writing the source. A rejected
report contains the first construction failure. A constructible report contains
weapon/bin counts and any authored-to-normalized ammunition changes, including
inferred half-ton bins. The Rust API is `check_battle_template(&template)`.

The native operation requires Wizard access. Lua follows the existing callback
and bounded asset-access rules and returns detached data. Syntax, missing-file
and unsafe-path errors remain asset errors; unsupported equipment or chassis
rules are reported as construction rejection. Checking a template does not
activate simulation or prove complete gameplay support. Tests verify parity,
source/world preservation, detached results and access/path restrictions.

For a directory-wide check, run
`cargo run --example btech_template_audit -- <mech-directory>`.
The example emits deterministic JSON with all regular-file outcomes and rejection
counts, using the same bounded reader and construction checker. Failures identify
the first blocker, so a later audit may expose additional missing capabilities.

### Repeated technology declarations

Repeated `Specials` records accumulate a case-insensitive union of technology
flags, matching the reference loader's additive behavior. First-occurrence
spelling and order are retained; empty records and `-` add nothing and never
clear existing technology. Unknown flags remain visible and fail construction
validation instead of being silently discarded. Duplicate scalar fields and
sections retain their existing rejection behavior. Tests cover additive fields,
case-insensitive duplicates, empty records and unsupported flags. Reference:
`unit/template_load.c`, Specials handling.

### Small cockpit control and mass

The `SMCPIT` construction flag reduces cockpit mass from three to two tons
while the head survives. It adds one to piloting rolls and the standing target.
This is a construction penalty, exposed as `BattlePilotingCheck.cockpit`, and
is kept separate from physical damage so undamaged units do not acquire
additional damage-triggered landing checks. The shared piloting path applies
it to ground, airborne and recoil checks without changing the base pilot skill.
The current conventional head-system validation remains in effect.
Tests cover mass, unchanged mobility, damaged and undamaged control targets,
standing targets and exact dice replay after persistence. Reference facts:
`unit/mech_consistency.c` and `unit/mech_identity.c::mech_pilot_skill_roll_target`.

### Ammunition construction diagnostics

Construction-time ammunition failures include their section, one-based critical
slot and equipment name even when rejected before loadout resolution. Unknown
ammunition modes additionally show their flags and weapon, so `template-check`
and `btech.template.check` identify the bin that needs unsupported behavior.

### First-slot weapon modes

A multi-slot weapon may declare its modes on its first critical alone, as in
bundled assets such as ARC-5R, or repeat them on its continuation slots. Empty
continuation modes preserve the primary's selection. Equipment identity, data
and brand must still agree, and nonempty conflicting continuation modes are
rejected. Mount numbering and slot ownership are unchanged. Tests cover LB-X
cluster/rear mounts, spent rockets, one-shot LRMs and targeting-computer flags,
including construction, readiness and serialized restoration.

This also supports Artemis launcher declarations. The reference loader retains
per-slot modes, while combat reads the selected weapon's first critical.

### Artemis controller construction and inspection

`ArtemisIV` is represented as a one-slot, one-ton system that can receive
critical damage. Its template data is a one-based launcher-primary slot;
`-` and zero are unassigned. A controller can link to a missile mount in its
own section. Mechs also permit head-to-center-torso links; ground vehicles permit
rear-to-turret links. VTOLs require local links, including stationary VTOLs.
Dangling links remain visible
and provide no guidance. The `ArtemisIV` chassis flag alone installs nothing.

`BattleUnit::artemis_controllers()`, `BattleVehicle::artemis_controllers()` and
Lua `btech.unit.state(id).artemis`
expose controller location, raw link, resolved weapon indices and availability.
`artemis_operational(index)` checks for a surviving linked controller. Critical
loss and section destruction disable the controller; Mech section flooding also
disables it. Critical losses
retain its installed mass; lost sections remove it. Tactical critical damage
reports the Artemis destruction notice.

Controller links and availability are derived from the owned definition and
ordinary damage state; no duplicate persistent controller state is introduced.
Tests cover local and head links, unassigned/dangling links, invalid data,
critical and flood disablement, mass and serialized restoration.
Compatible ammunition selection, missile bonuses and controller-loss mode
clearing are described in the firing section below.
Reference: `unit/template_load.c`, `unit/mech_ammunition.c`,
`unit/mech_consistency.c` and `combat/crit_mechs.c`.

### Artemis-compatible ammunition and firing

`Artemis/Mine` is now accepted as the compatible-ammunition selection for missile
mounts and bins. The selected bins alone supply the launcher; ordinary and
compatible supplies are not silently substituted. Initial template selection is
preserved, even with an unassigned controller, as in ARC-5R.

Native `artemis <selection>` and Lua `btech.unit.artemis(unit, pilot, index)`
share pilot, power, intact-weapon, recycle, manual-feed-jam and live-controller
checks across supported Mechs and vehicles. One-shot
and rocket mode changes are rejected. Mode changes participate in ordinary
callback rollback and persistence. Firing uses a +2 missile-table modifier,
combined with the glancing -4 before clamping; Streak still hits with its whole
salvo. No extra hit modifier or ammunition cost is applied.

The reference's critical-handler indexing differs from its controller lookup:
critical loss clears Artemis mode on a primary at the raw, zero-based data slot
in the controller's own section. A zero/unassigned controller therefore clears
slot zero; this is covered by the unchanged ARC-5R test. Positive link lookup
remains one-based, including the head-to-center exception. Rust preserves this
observable distinction instead of silently repairing stored links.

Tests cover missile tables, glancing boundaries, controller critical mode
clearing, compatible supply, native/Lua firing parity, rollback and persistence.
ECM/Angel suppression awaits implementation of electronic-warfare state; units
requiring those systems remain rejected by construction.

### Hotload metadata on ammunition bins

The reference loader accepts `Hotload` as a bin fire flag, but hotload firing
and jam checks read the weapon's critical, not the supply bin. Rust now retains
this flag as `AmmunitionBin.hotload` (also exposed in Lua loadout inspection)
without changing the bin's ammunition type, capacity, readiness or explosion
contents. Half-ton inference remains driven by quantity and `Halfton`.
Compatible rounds and the retained flag are independent; duplicates and unknown
flags remain rejected.

Tests compare flagged and unflagged supplies for every supported ammunition
weapon, including mass, readiness, critical explosions and restoration, plus
inferred half-ton Artemis rounds. Launcher hotloading is described below; it remains independent of bin flags. Reference facts:
`unit/template_load.c`, `unit/mech_weapons.c::full_ammo`,
`combat/mech_fire_resolution.c` and `combat/mech_combat_missile.c`.

### Ammunition-feed jam state

Weapon readiness now includes `jammed`, independent of `intact`, ammunition
quantity and recycle time. Native `weapons` displays this state. A jam blocks
firing and fire/ammunition mode changes without spending heat, supply or dice.
It persists across restart and is not cleared by ordinary recycling.

The Rust domain primitives `jam_weapon(index)`, `weapon_jammed(index)` and
`clear_weapon_jam(index)` validate mount identity; creating a jam additionally
requires an intact ammunition-fed weapon. These are transaction building blocks,
not player recovery commands. Saved jams referring to absent or non-ammunition
weapons fail world validation. Tests cover persistence, readiness, native/Lua
inspection and control rejection, unchanged expenditure, clearing and malformed
saved state. Timed player unjamming and hotload-triggered jams are described below. Reference entry points: `combat/mech_fire_resolution.c` and
`combat/mech_weapon_modes.c`.

### Timed feed recovery

Native `unjam <selection>` and Lua `btech.unit.unjam(unit, pilot, index)` share
an atomic 60-second recovery start. Only one attempt is allowed per unit;
walking-or-slower speed, ground state and no recycling weapons are required.
Readiness, jump and speed controls enforce the active-attempt restrictions.
Lua unit state exposes `unjam` with weapon index and remaining seconds.

The server advances recovery inside its ordinary world-save/effect boundary.
At expiry, current power, consciousness, weapon condition and matching supply
are checked. Empty supply clears the jam without rolling. Otherwise an ordinary
piloting check clears it on success and ejects one round from a matching bin,
preferentially in the mount section. Failure requires a new attempt. The timer
ends at expiry, including shutdown or destroyed-weapon cancellation. Saved
timers must refer to ammunition-fed mounts and have 1 through 60 seconds left.
The domain returns cockpit feedback for the adapter to stage.

Successful non-rotary checks award the configured piloting XP for eligible
character pilots. Rotary mounts use their separate gunnery check without a
control-check XP award. `advance_battle_unjamming_action` publishes recovery
feedback and `MechPilotXP` diagnostics atomically; the pure-world advancement
rejects active character attempts. The server stages the same result inside
its complete tick transaction, including observer shell-ejection feedback.

Tests cover native/Lua parity, restrictions, all sixty ticks,
success/failure/empty/shutdown outcomes, both piloting skills, rotary and
ineligible pilots, save/reload at the final second, channel delivery rollback
and server database failure followed by a successful retry.
Reference: `combat/mech_weapon_modes.c::mech_unjamammo_func` and
`unit/mech_events.c::mech_unjam_ammo_event`.

### Launcher hotloading

`Hotload` weapon modes and native `hotload <selection>` / Lua
`btech.unit.hotload(unit, pilot, index)` now support IS LRM, extended LRM and
long-range dead-fire launchers. Mode changes require an intact, recycled,
unjammed launcher; one-shot mode changes are rejected. Ammunition selection
remains independent, including Artemis rounds.

Hotloading removes minimum-range penalties unless `hotloadaddshalfbthmod` is
set, in which case the raw-distance half modifier applies. `BattleAimRules`
exposes that option explicitly. Cluster resolution consumes three dice and uses
the lowest two, before Artemis and glancing adjustments. Existing ELRM/dead-fire
attack-dice rules remain in effect. An attack roll of two or three records a
feed jam, consumes shooter dice, and spends no ammunition, heat or recycle;
`BattleShotReport.jammed` distinguishes it from a failed Streak lock. Native/Lua
feedback reports the jam, and the ordinary timed unjamming path recovers it.

The first critical on an intact hotloaded launcher explodes for a full salvo's
damage if usable ordinary ammunition exists. Compatible special rounds alone
do not trigger this effect, and no bin round is spent. The whole mount is lost;
internal damage uses the shared cascade and CASE path. This explosion does not
apply the Gauss weapon's extra personal-injury effect. Tests cover range options,
construction/restoration, supply-dependent critical damage, tactical explosion
replay, native/Lua hits and jams, three-die cluster rolls and recovery.

Reference: `combat/mech_bth.c`, `combat/mech_combat_missile.c`,
`combat/mech_fire_resolution.c`, `combat/crit_weapons.c` and
`combat/mech_weapon_modes.c`. Broader observer broadcasts and XP remain on the
integration coverage list.

Recovery timers skip units pending deletion, matching other committed BattleTech
timers. A deletion-pending unit at expiry must not fail a piloting availability
check and roll back unrelated units' progress. Tests cover this case, atomic
rejection of zero/oversized countdowns and invalid weapon references after an
earlier unit has already advanced in the candidate, and saved replay of
weapon-destruction cancellation without consuming ammunition or dice.

### Ultra autocannon single-shot construction

IS Ultra AC/2, AC/5, AC/10 and AC/20 now have catalog identities, profiles,
matching ammunition, mass, ballistic skill and recycle feedback. Their normal
mode uses the common direct-fire path, including targeting-computer assistance,
glancing damage, ammunition hazards and native/Lua transactions. Ultra AC/20
also supports explicit split criticals.

The existing catalog snapshot, split-mount tests and native/Lua direct-fire
restart tests cover these weapons. Double-shot ammunition/heat accounting and
loader failure are described below. Reference catalog facts: `unit/weapons_catalogue.c` and
`unit/weapons_vrt.h`.

### Shared ammunition draw planning

`BattleUnit::ammunition_feed(index, rounds)` returns ordered
`BattleAmmunitionDraw` records without mutating supply or consuming dice. It
prefers bins in the mount section, then canonical section/slot order, and skips
empty, destroyed, flooded or differently selected ammunition. Each record
contains a bin index and bounded quantity. If supply is short, the plan returns
only the available rounds so the calling firing mode can apply its specified
fallback atomically.

Ordinary firing and timed unjamming now share this selector instead of keeping
separate bin-priority implementations. Tests cover two rounds crossing a bin
boundary, shortages, zero/oversized requests, damage, mode isolation and saved
restoration. Ultra firing uses this planner for two-round expenditure and
shortage fallback. Reference: `unit/mech_systems.c::ammunition_check` and
`combat/mech_ammunition_decrement.c`.


### Ultra double-shot firing

IS Ultra AC/2/5/10/20 accept `UltraMode` on the primary weapon critical and
persist the live `ultra` mode. Native `ultra <selection>` and Lua
`btech.unit.ultra(unit, pilot, index)` share authorization, readiness, cockpit
feedback and transaction rollback. Inspection labels the selected mode.

Supply checks request two compatible rounds, including draws across bins. A
one-round supply resets the mode to normal before resolving loader failure or
expenditure. An active double-shot attack roll of two permanently destroys all
mount criticals, consuming only the attack dice: no rounds, heat or recycle.
This does not create a recoverable feed jam. Other double-shot attempts spend
two rounds and twice the catalog heat, including misses. Successful attacks
roll the two-shell table (2–7: one; 8–12: two), with independent full-damage
locations. Glancing shifts that table roll down four instead of halving damage.

`BattleWeaponUse.ammunition` reports each actual `(bin_index, rounds)` draw;
`fire_mode` reports the effective mode after fallback. Native and Lua use this
same result. The single-bin report field has been replaced rather than retained
as a compatibility layer. `BattleShotReport.loader_destroyed` distinguishes
permanent loader failure from hotload `jammed` state.

Tests cover all four weapons, same-bin and cross-bin supply, hits and misses,
roll-two failure versus single-round fallback, complete mount destruction,
rollback and persistence replay. Reference behavior was characterized from
`combat/mech_weapon_modes.c`, `unit/mech_systems.c`,
`combat/mech_fire_resolution.c`, `combat/mech_hit_resolution.c`, and the
2-shell table in `unit/mech_build.c`. RAC, other rapid-fire families, XP and
general observer combat feedback remain separate coverage gaps.


### Conventional autocannon rapid fire

IS AC/2/5/10/20 and Light AC/2/5 accept `RapidFire` and the live `rapid` mode.
Native `rapidfire <selection>` and Lua `btech.unit.rapidfire(unit, pilot, index)`
share pilot/readiness checks, mode feedback and callback rollback. Both
rapid-fire families use common two-round supply fallback, heat expenditure and
independent full-damage shell grouping, including glancing cluster adjustment.

Rapid fire differs from Ultra on failure: attack rolls three and four jam the
feed without expenditure or recycle; a two destroys the entire mount and
applies catalog damage directly to its mounting section's internal structure.
The initial packet does not transfer beyond that section, matching the
reference's normal transfer flag on this internal-only request. Nested critical
explosions retain their own transfer and crew effects. Surviving selected bins
are decremented after that damage cascade; this failure adds no heat or recycle.
The shot report contains the applied `misload` impact and actual surviving-bin
draws. A one-round supply resets to normal before these failure rules apply.

Reference characterization: `combat/mech_weapon_modes.c::mech_rapidfire`,
`combat/mech_fire_resolution.c` RFAC failure and heat branches,
`combat/mech_damage.c` internal transfer branch, and `unit/weapons_catalogue.c`
RFAC flags. Tests cover all six weapons, supply boundaries, both jam rolls,
catastrophic internal damage, misses, rollback and persistence replay. Observer
explosion broadcasts, experience awards, RAC and gatling modes remain open.


### Rotary autocannon single-shot equipment

IS Rotary AC/2 and AC/5 now have typed catalog entries, matching ammunition,
ballistic skill, mass, critical-slot counts and recycle feedback. Single shots
use the ordinary direct-fire path and its native/Lua transactions, damage,
ammunition hazards, critical losses and saved recycle state. The catalog
snapshot records the reference profiles, including 15/22-second recycle times.
Both weapons are included in the common direct-fire rollback/restart test.

This does not enable rotary burst flags or the `rac` control. Follow-up work
must add one/two/four/six selection, burst-specific jam thresholds, supply
fallback, shell tables and rotary recovery. In particular, rotary unjamming
uses a gunnery target plus three rather than the ordinary piloting check.
Reference equipment facts: `unit/weapons_catalogue.c` and `unit/weapons_vrt.h`;
mode/failure/recovery entry points: `combat/mech_weapon_modes.c::mech_rac`,
`unit/mech_systems.c::ammunition_check`, `combat/mech_fire_resolution.c` and
`unit/mech_events.c::mech_unjam_ammo_event`.


### Rotary feed recovery

Timed `unjam` recovery now selects the current unit gunnery target plus three
for IS rotary autocannons. This uses the configured extended-gunnery setting
independently of extended piloting, and consumes exactly two dice even for a
prone unit. A disconnected pilot uses the ordinary fallback gunnery target six,
so the recovery target is nine. Ordinary feed recovery retains its piloting
check. The success/failure messages, one discarded round on success, timer,
empty-supply handling and atomic world transaction are shared.

Boundary tests cover both rotary weapons, generic versus ballistic skill,
connected and disconnected pilots, prone recovery and last-second restart.
They verify exact RNG advancement, inventory, success/failure and unchanged
skill experience. The timer API now takes both configured skill switches, and
the server passes them separately. Pilot-only roll-detail messages and observer
shell-ejection broadcasts remain open alongside rotary burst selection/firing.
Reference: `unit/mech_events.c::mech_unjam_ammo_event`.


### Rotary burst selection and firing

IS rotary autocannons now accept `Rotary_TwoShot`, `Rotary_FourShot` and
`Rotary_SixShot` template flags, with persistent `rotary2`, `rotary4` and
`rotary6` modes. Native `rac <selection> [rate]` selects one, two, four or six
rounds. Its optional rate follows the reference's first-character parsing;
omission or another initial character selects one. Lua
`btech.unit.rac(unit, pilot, index, rounds)` validates an explicit 1/2/4/6 rate
(default one) and returns whether the setting changed. Repeating a selection
keeps it enabled and reports that it was already selected. Both surfaces share
readiness, pilot checks, selection feedback and transaction rollback.

The shared draw planner supplies the complete burst across compatible bins.
Insufficient supply resets the saved mode to single-shot before jam resolution.
Two-round bursts jam on two, four-round bursts on two/three, and six-round
bursts on two through four. Jams spend no supply, heat or recycle and do not
destroy the weapon. Other attempts spend the chosen number of rounds and
multiply catalog heat by that number, including misses. Hits use SRM-2/4/6
cluster counts with independently located full-damage shells; glancing shifts
the cluster roll down four with a one-shell floor. Ordinary single shots retain
direct-hit behavior. The existing rotary gunnery-based recovery clears jams.

Table tests cover every cluster roll and glancing boundary. Integration tests
cover both weapons at all three rates, template modes, repeated selection,
cross-bin supply, empty/short supply, jam thresholds, misses, callback rollback
and saved firing replay. Reference characterization: `combat/mech_weapon_modes.c`,
`unit/mech_systems.c`, `combat/mech_ammunition_decrement.c`,
`combat/mech_fire_resolution.c`, and `combat/mech_hit_resolution.c`.
Clan rotary equipment, additional observer feedback and XP remain open.


### Gatling machine-gun fire

IS MachineGun and HeavyMachineGun accept the reference `Gattling` template flag
and live `gatling` mode. Native `gattling <selection>` and Lua
`btech.unit.gattling(unit, pilot, index)` share mode guards, cockpit feedback and
transaction rollback. A gatling attempt rolls one die before the attack dice.
The supply-limited value is the lesser of that die and complete available
three-round groups, with a floor of one. It supplies both heat and one packet's
pre-glancing damage. Expenditure draws up to three times that value across bins;
with one or two rounds left it spends the remaining supply for one heat/damage.
A miss spends the same heat/ammunition; empty supply rejects atomically. There
is no gatling feed-jam trigger or mode reset on shortage.

`BattleWeaponUse.gatling_damage` records that prepared value. The hit resolver
now receives the shared expenditure report rather than a growing positional
tuple, so it uses the same mode and rolled value as supply/heat accounting.
The standalone expenditure operation also prepares one roll and commits its
candidate atomically. Glancing halves the damage packet rounded up, while heat
and ammunition remain unchanged. Native/Lua rollback, exact dice ordering,
low-supply cases, cross-bin draws, hits/misses and restart replay are tested for
both supported machine guns.

Reference behavior: `combat/mech_fire_preparation.c` prepares the die,
`unit/mech_systems.c::ammunition_check` limits it, and
`combat/mech_ammunition_decrement.c`, `combat/mech_fire_resolution.c` and
`combat/mech_hit_resolution.c` consume and apply it. Clan machine guns,
additional observer feedback and XP remain open.


### Shared weapon control guards

`weapon_controls.rs` owns the common pilot, running-power, intact-mount,
feed-jam and recycle checks for eight fire/ammunition controls. Controls retain
their own equipment eligibility, one-shot/Artemis restrictions and cockpit
messages. A shared setter stores normal firing implicitly and reports repeated
selection; toggles use that setter while rotary selection remains idempotent.
This keeps native and Lua on the same domain operations and avoids copying
control guards when new equipment is added. Public commands, Lua functions and
saved-state representations are unchanged by this consolidation.


### Precision autocannon ammunition

IS AC/2/5/10/20 and Light AC/2/5 accept `Precision` ammunition and weapon flags,
with persistent live `precision` selection. Native `precision <selection>` and
Lua `btech.unit.precision(unit, pilot, index)` share authorization/readiness,
feedback and rollback. Precision supply remains separate from normal rounds and
composes with rapid firing. Weapon inspection displays both active selections.

Precision bins hold half the catalog round count, rounded down. `Halfton` and
Precision combine as an OR, so both flags still produce half capacity, not a
quarter. Factory loading normalizes counts; saved live depletion remains intact.
Mass and explosion damage continue to use live rounds and catalog values,
matching the reference's `ammo_weight` and ordinary damage behavior.

Aim subtracts two from the complete target-movement modifier and clamps it at
zero. This intentionally also clamps negative movement modifiers from immobile
or prone targets, as the reference applies Precision after computing that whole
modifier. Intrinsic weapon accuracy, damage, heat and recycling are unchanged.
Tests cover all six weapons, half-ton interaction, saved contents, native/Lua
selection, separate supply, rapid-fire composition and persisted shot replay.
Reference: `combat/mech_weapon_modes.c::mech_precision`, `combat/mech_bth.c`,
`combat/mech_bth_movement.c`, `unit/mech_weapons.c::full_ammo`, and
`unit/mech_consistency.c::ammo_weight`.


### Flechette ammunition against armored targets

Conventional IS AC/2/5/10/20 and Light AC/2/5 now accept `Flechette` template
and live ammunition selection. Native `flechette <selection>` and Lua
`btech.unit.flechette(unit, pilot, index)` share the weapon-control guards and
ammunition state updater with Precision selection. Ordinary-capacity bins,
explicit half-ton sizing, isolated supply and saved live depletion follow the
common ammunition path. Inspection shows the selected rounds alongside rapid
fire when both apply.

The supported constructed targets are armored bipeds. Flechette damage against
them is half the ordinary shell damage, rounded down. Normal glancing hits then
halve that result, rounded up. Rapid-fire shells each retain the halved damage;
glancing changes their cluster count instead. Aim, heat, expenditure, recycling
and ammunition explosion potential remain ordinary. Tests cover all six weapons,
bin sizing and persistence, damage/glancing boundaries, and shared native/Lua
Precision/Flechette shot replay with rapid-fire composition.

Infantry and battle-armor construction/combat remain unsupported. Their
Flechette target-class/terrain damage rules must be added when those units are
implemented; this armored-target support does not claim those effects.
Reference: `combat/mech_weapon_modes.c::mech_flechette`,
`combat/mech_hit_resolution.c::mech_hit_damage_determine`, and
`unit/mech_weapons.c::full_ammo`.

Ammunition compatibility and template flag decoding share one domain policy across
construction, saved-unit validation, controls and hit grouping. All special
ammunition controls use the same state update, with normal selection stored
implicitly. Artemis retains its live controller and disposable-launcher guards.
Native/Lua tests also switch directly between Precision and Flechette, including
selection without stocked rounds and preservation of the independent rapid mode.

### Armor-piercing autocannon ammunition

The six supported conventional/light IS autocannons accept `AP` template flags,
`armorpiercing` cockpit selection and `btech.unit.armorpiercing` (returning
`armor_piercing` or `normal`). AP bins hold half ordinary capacity; `Halfton`
does not halve that again. Live supply remains separate from ordinary ammunition.
AP adds one to aim and retains ordinary shell damage, heat and burst behavior.

The shared impact resolver checks front/rear armor after each packet: AP can
roll criticals if the packet leaves no internal overflow and less than half the
original armor remains. Its 2d6 critical roll is reduced by four for AC/2 and
LightAC/2, three for AC/5 and LightAC/5, two for AC/10, and one for AC/20.
Ordinary through-armor and penetration criticals retain their own rules; nested
explosions do not inherit AP. Native/Lua rollback, cross-mode selection, bin
capacity, rapid fire, threshold dice consumption and saved shot replay are tested.

### Caseless autocannon ammunition

The six conventional/light IS autocannons support `Caseless` template flags,
`caseless` cockpit selection and `btech.unit.caseless`. Ordinary caseless bins
hold twice the normal capacity; an explicit or inferred `Halfton` bin retains
half the ordinary capacity. Damage and aim are ordinary, and rapid fire remains
an independent selection.

Attack rolls two/three jam and consume a second shooter 2d6 roll. Seven or less
leaves a recoverable jam without expenditure; eight or more destroys the mount,
causes internal damage in its section and spends the selected firing quantity
from surviving ammunition bins. This precedes rapid-fire failure checks. Short
supply persists single-shot fallback even on a caseless failure. Failures add no
heat or recycle, and their nested explosions use the existing damage rules.
Native/Lua failure feedback, rollback, boundary rolls, supply, replay and saved
jam recovery are tested. General observer explosion broadcasts remain open.

### Incendiary autocannon ammunition

The six conventional/light IS autocannons support `Incendiary` template flags,
`incendiary` cockpit selection and `btech.unit.incendiary`. Bin capacity, aim and
damage against supported armored bipeds are ordinary; half-ton bins and rapid
fire compose with the shared ammunition path. No additional target heat is added.

A first critical on an intact recycling mount ignites its loaded rounds when a
nonempty, available matching incendiary bin exists. The whole mount is lost and
catalog shell damage enters the shared internal explosion cascade. Ignition does
not spend ammunition from the supplying bin. Tests cover recycling, depletion,
wrong ammunition, flooded supply, whole-mount loss and tactical explosion damage
and notices. The infantry damage bonus and observer broadcasts remain open.

### Direct-fire misload observers

Rapid-fire misloads and caseless propellant ignition now broadcast to occupants
of running units that have an acquired, currently visible contact with the shooter.
The audience is captured before the shot changes posture or damage. Cockpit and
observer messages share native/Lua transaction staging, so callback rollback
publishes neither. Audience queries are read-only and consume no acquisition dice.
Subject, unavailable units, lost contacts and other maps are excluded. The current
contact display uses the Rust unit number and name. Critical/thermal explosions
and other battlefield broadcasts still need their event-specific integration.

### Shared critical and thermal explosion broadcasts

Tactical ammunition and weapon explosions now resolve their observer audience
inside the shared damage cascade before internal damage changes visibility.
Ammunition, Gauss discharge, hotloaded-launcher and incendiary ignition messages
flow through the same notice list as cockpit effects. Heat-triggered ammunition
explosions reuse this resolver; native/Lua actions and server ticks retain their
existing commit/rollback boundary for those notices.

Battle notices own their text so formatted observer messages require no separate
event bridge. Weapon-destruction cockpit text comes from catalog names and the
resolved explosion damage. Tests cover observed incendiary criticals and an
ammunition explosion that destroys its subject, including restart replay. Other
battlefield event broadcasts and complete sensor/identity parity remain open.

### Unjamming audiences

Recovery expiry now returns typed player/cockpit recipients. Rotary checks send
the pilot the gunnery target and actual roll; other rolled checks send piloting
details. With no assigned pilot, those details fall back to the cockpit. Prone
automatic recovery, empty supply and cancelled attempts do not fabricate rolls.
Successful recovery broadcasts `ejects a mangled shell!` to eligible observers;
failure and empty supply do not. Server tick staging preserves the existing
rollback boundary, and direct player routing excludes cockpit passengers.
XP and other remaining skill-system behavior are still pending.

### Third-party firing feedback

Launched direct shots now report hits/misses to third-party observers using a
pre-shot visibility snapshot. An observer seeing both units receives both names
and the outcome; seeing only the shooter reveals firing at something; seeing
only the target names that target and hides the shooter. Neither participant is
included in this audience, and observers seeing neither receive no message.
Shutdown/unavailable units are excluded. Loader failures and failed Streak locks
do not emit ordinary firing broadcasts. Native/Lua output, rollback and restart
are tested for hits and misses across all participant-visibility combinations.
Hex targeting and full sensor/contact identity parity remain open.

Single-contact inspection, full contact displays and observer feedback now share
one live eligibility implementation. `visible_battle_contact` returns one acquired
visible target or `None`; it retains observer validation and never acquires a
contact. Firing audiences check only their two participants, and filtered native
`contacts` queries no longer construct or sort every other known contact.

### Power-state observers

Completed startup now emits `powers up!` to current eligible observers after the
unit enters the running state. Intermediate stages, subsequent ticks and aborted
startup do not repeat it. Ground shutdown above 10.75 kph emits
`stops in mid-motion, and falls!` before resolving the fall. Stationary shutdown,
reverse motion and the airborne free-fall branch do not use that message.
Completion/restart timing, exact speed boundaries and native/Lua shutdown output
and rollback have tests. Other startup hazards and unit classes remain pending.

Successful standing completion now broadcasts `stands up!` to current eligible
observers. Failed-attempt recovery remains a cockpit-only event. The saved timer
still completes while shut down, and completion consumes no dice. Tests cover
both timer kinds, running/shut-down units, delayed and final-second replay, and
absence of duplicate completion messages.

Free-fall impact now broadcasts `hits the ground!` using visibility captured before
landing damage resolves. The cockpit message precedes observer feedback and damage
notices, including when the impact destroys the unit. Countdown ticks remain quiet.
Tests cover light/destructive impacts, shut-down observers, failed-impact rollback,
saved final-second replay and absence of duplicate landing feedback.

Jump launch returns cockpit and observer notices in one domain result, consumed by
both native and Lua adapters. Eligible observers see `engages jumpjets!`, normal
completion emits `lands gracefully.`, and failed early landing emits a crash
message before fall damage. Missing-leg, actuator and gyro landing failures each
have their own observer message. Collision/avoidance events suppress graceful
completion. Tests cover recipient routing, callback rollback, saved replay,
automatic completion and all three supported damaged-gear failure paths.

Stand attempts now capture their complete ordered notice stream in the domain
result. Observers see `attempts to stand up.` before the cockpit outcome, and a
failed attempt adds `falls down!` before its damage notices. Native and Lua stage
the same stream. Normal/anyway/careful modes, success/failure, shut-down observers,
read-only checks, rejected repeat attempts, callback rollback and saved replay
have coverage. Lua stand results include the detached `notices` array.

Crowding collisions and avoidance now notify third-party observers using the same
independent participant visibility snapshot as firing. Hidden participants appear
as `Someone`/`someone`; the two participants retain their cockpit messages.
Ground entries report bumps, while jump/fall entries report landings. Failed
non-ground avoidance also broadcasts the fall when the jump movement modifier
is nonzero. Messages are captured before collision damage. Tests cover each entry
and collision mode, partial visibility, shut-down observers, failed-damage rollback,
saved replay and native/Lua early-landing output and callback rollback.

Thermal shutdown now broadcasts `falls from the sky!` for airborne units and
`stops in mid-motion!` for grounded units, including stationary ones. Failed
balance above one movement point adds `falls down!` before damage resolution.
Successful overrides and later checks of already stopped units do not repeat the
shutdown broadcast. Ground speed/reverse boundaries, observer shutdown, airborne
falls and saved replay have regression coverage through the shared thermal report.

`btech.unit.rottorso(unit, pilot, direction)` and
`btech.unit.fliparms(unit, pilot)` now expose the existing torso and arm controls
to Lua. Torso directions accept case-insensitive left/right/center and l/r/c.
Both calls return true on success and stage the native cockpit message. They use
the same pilot, power, posture, chassis and rotation-limit guards, and callback
failure restores pose and output. These controls remain cockpit-only. Tests cover
both flip directions, centering, limits, invalid/caught calls and saved replay.

Speed and heading controls now return their confirmation from the domain setter.
Native and Lua adapters stage it for all cockpit occupants in the same transaction
as the requested motion change. Lua still returns true. Heading normalization,
named native speeds versus numeric Lua values, unchanged controls, pilot/passenger
routing, exclusion of outside listeners, saved replay and callback rollback have
coverage. Control changes consume no dice and movement still advances on ticks.

Ice and bridge fracture results now include observer feedback captured before
terrain replacement can hide surface occupants. Triggered downward ice breaks
announce the break and, at nonzero depth, disappearance into water; neighboring
falls report swimming in their existing resolution order. Upward breakout reports
only a currently visible breaker, preserving concealment while still submerged.
Non-triggered ice/bridge failures include their distinct occupant messages.
Zero-depth ice, bridge height selection, powered-off observers, failed-fracture
rollback, saved replay and upward/landing ordering have coverage. Weapon-triggered
terrain blast messages and trigger probabilities remain pending.

Stagger reports now own their ordered notice stream. Rolling retain/consume modes
announce light, increased or violent staggering at levels one, two or three-plus,
including when the pilot retains balance. A failed rolling check broadcasts
`tumbles over, staggered by the damage!`; traditional mode broadcasts only a failed
fall as `falls down, staggered by the damage!`. Visibility is captured before fall
damage. Severity boundaries, all three modes, successful/failed checks, observer
shutdown, pending countdowns, saved replay and no repeated immediate check are tested.

Immediate damage-induced balance reports now capture observer fall feedback before
posture, power or submersion changes visibility. Failed gyro/leg-actuator checks,
destroyed gyros and lost legs use their distinct ground messages; airborne gyro or
last-jet loss reports `falls from the sky!`. Successful checks and deferred airborne
actuator damage produce no balance-loss message. Cause-specific ground tests cover visibility and saved
replay; airborne gyro/jet tests cover retained flight, forced falls and restart.

Engine criticals now broadcast section-specific black smoke when the unit was
running and intact immediately before the hit. Capturing visibility before the
critical preserves the final smoke message when that hit destroys the engine.
Shut-down engines and repeated losses do not emit it. Tests cover all three engine
hit stages, observer shutdown, fatal-hit visibility, saved replay and repeated-slot
idempotence through the shared tactical damage path.

Running units now show visible hip locking and leg-actuator twisting when a new
critical is applied. Arm actuator hits do not use these messages, and leg-actuator
feedback is suppressed when that leg's hip is already unavailable. Mechanical
damage feedback precedes any balance-loss message; airborne actuator damage can
therefore remain visible while flight continues. The shared component visibility
helper also handles engine smoke. Both legs, arms, stopped units/observers, hip
loss, repeated slots, airborne behavior and saved replay have regression coverage.

New limb criticals now also notify cockpit occupants: frozen shoulders/hips,
side-specific upper/lower arm and hand actuators, and leg actuators. These
messages remain available while shut down and after prior hip loss, independently
of observer visibility. Hip observer feedback precedes the cockpit message;
other actuator cockpit messages precede observers, and both precede balance
resolution. The limb matrix covers both arms/legs and saved replay.

Core criticals now report engine shielding damage and engine destruction, sensor
damage/destruction, life-support loss and ordinary gyro damage/destruction to
cockpit occupants. Hardened gyro stages retain their extra protected hit. The
shared critical transaction emits these messages before balance resolution,
including when the reactor is off. Seeded stage tests cover saved replay, fatal
engine loss and idempotent repeated slots.

Gyro criticals now broadcast the initial buckling/screech before cockpit damage
and balance feedback. Hardened gyros have a distinct protected first-hit effect
that remains visible with the engine off; their next hit uses the ordinary
buckling effect only while running. Later gyro hits do not repeat either cue.
A powered/stopped, visible/hidden, ordinary/hardened matrix covers all four slots
and saved replay; airborne tests verify screech-before-fall ordering.

Fatal cockpit criticals now stage the occupant destruction message followed by
the visible spasm/stillness cue. Observer visibility is captured before unit
destruction clears power and pilot assignment; stopped units can also show the
effect. Tactical player objects remain available after the unit's cockpit claim
is released. Tests cover power/visibility combinations, saved replay, repeated
critical loss and atomic rejection of unsupported in-character casualties.
Attacker-specific cockpit feedback and in-character casualty lifecycle remain open.

Conventional weapon criticals now distinguish a newly destroyed mount from a
later hit on its non-working slots. Existing explosive-weapon messages keep their
separate explosion detail. Heat-sink losses notify occupants and show green mist
to pre-loss observers, including stopped units. Grouped double sinks emit one
loss and repeated unavailable slots do not repeat it. Tests cover single/double
sinks, intact/broken mounts, power, visibility and saved replay. Enhanced partial
weapon-damage rules remain outside this conventional critical path.

Jump-jet criticals now show section-specific plasma flares while the reactor is
running. Visibility is captured before the loss; the flare precedes cockpit
feedback and a last-jet fall. The equipment matrix covers shutdown, visibility
and repeated slots, while airborne tests replay both surviving flight and
immediate last-jet falls from saved state.

`ImprovedJJ_Tech` selects two contiguous matching jump-jet slots per MP. Declared
jump speed must match installed pairs and remain within the existing chassis
speed limit. Each slot retains ordinary jet mass, so a complete improved jet
weighs twice an ordinary jet. Either critical slot disables the entire pair;
flooding and section destruction count one lost jet per pair. Jump capacity,
heat and collision thrust share that effective loss count.

Construction rejects incomplete/cross-section/mixed-brand pairs and partial
saved pair losses. Tests cover all ten slots of a five-jet fixture, native/Lua
launch and rollback, mass, heat, flooding, severed sections, surviving flight,
last-jet falls, duplicate loss and saved replay.

`IS.HeavyFlamer` now uses the conventional firing pipeline: one slot/ton,
5 firing heat, 4 damage, ranges 2/4/6, ten fuel rounds per ton and a 15-second
recycle. It uses ballistic gunnery and is excluded from targeting-computer
assistance. Fuel-bin explosion damage follows the remaining rounds.

Normal and heat-transfer modes share the flamer controls and template `Heat`
flag. Heat mode transfers four heat on a hit, including a glance, and consumes
fuel even on a miss. Native/Lua firing, mode initialization/toggling, empty and
last-round fuel, callback rollback, saved replay and hit/glance/miss boundaries
are covered. The asset audit still constructs 433 of 1,745 files; this count is
construction coverage, not gameplay completion.

`IS.CoolantGun` uses ballistic gunnery, one slot/ton, 25 rounds per ton, ranges
1/2/3 and a 15-second recycle. A successful shot applies three points of cooling
instead of material damage; glancing hits retain full cooling and misses still
spend ammunition. The flamer heat-mode control selects self-application. In that
mode `fire 0` needs no target lock and explicit targets are redirected to the
firing unit. Ordinary weapons retain their self-fire prohibition.

Shot reports expose optional `cooling` separately from `heat_transfer`, and aim
reports identify coolant `self_target` without inventing an acquired contact.
Cooling may leave temporary negative stored weapon heat, which offsets firing
before the next heat sample; unused credit then clears at zero. Tests cover
external/self application, cold/fractional/hot storage, fuel exhaustion,
hit/glance/miss boundaries, callback rollback and saved replay before/after
cooling, including negative stored heat.


VehicleFlamer and VehicleHeavyFlamer now use the shared flamer firing rules:
ballistic skill, ammunition, normal damage or heat transfer, and exclusion from
targeting-computer assistance. Both carry 20 rounds per ton and recycle in ten
seconds. The light variant weighs half a ton, produces three heat and deals two
damage at ranges 1/2/3; the heavy variant weighs one ton, produces five heat and
deals four damage at ranges 2/4/6. Their names identify weapon variants; vehicle
chassis support remains a separate gate.


The acid thrower uses conventional ballistic direct damage: three damage and
three firing heat, ranges 1/2/3, two critical slots, 1.5 tons, ten rounds per ton
and a 25-second recycle. It permits targeting-computer assistance and has no
flamer heat mode. The reference exposes an acid identity helper but does not
dispatch a separate acid hit effect; this implementation likewise applies
ordinary material damage and conventional ammunition explosions.


IS plasma rifles apply ten direct damage, ten firing heat and a 30-second
recycle, using ballistic skill and ten rounds per ton. The two-slot mount weighs
six tons and has ranges 5/10/15. Its ammunition is inert; a bin critical removes
the rounds without an ammunition explosion.

Plasma heat is part of damage resolution. After the primary path resolves, each
completed section visit adds one d6 of target heat in transfer-unwind order.
Already-destroyed sections are skipped, and overflow beyond the core returns
before that final section's heat effect. Nested ammunition explosions do not
inherit plasma effects. Impact reports expose the applied rolls as
`plasma_heat`; ordinary damage reports contain an empty array. Heat rolls use
the target's persistent dice after damage and critical rolls.


Thunderbolt-5/10/15/20 launchers use missile gunnery and fire one missile with
5/10/15/20 damage. Their cluster table always yields one missile, including
glancing and Artemis-adjusted results. All four have minimum range five and
range brackets 6/12/18. Hotloading uses the shared minimum-range controls and
loaded-launcher critical explosion rules. Ammunition explosions use remaining
rounds times the missile payload. Their single missile still consumes the
normal missile cluster roll, preserving replay order.


HyperAC/2, HyperAC/5 and HyperAC/10 use conventional direct ballistic fire.
Their catalog ranges, minimum range, mass, ammunition and recycle differ from
ordinary autocannons. They do not carry the reference RFAC capability, so rapid
fire and RFAC ammunition modes remain unavailable. The HYPER catalog flag has
no separate combat handler in the inspected reference.


Supported Clan energy weapons now include ER large/medium/small/micro lasers,
ER PPCs, flamers, heavy large/medium/small lasers, large/medium/small/micro pulse
lasers, ER large/medium/small pulse lasers and the Clan plasma rifle. Each keeps
its catalog mass, heat, damage, range, slots and recycle time. Pulse weapons
apply -2 accuracy, heavy lasers +1, and Clan flamers share normal/heat controls
while weighing half a ton. The Clan plasma rifle is a beam weapon with ordinary
material damage and no IS plasma heat effect. These weapons can be installed on
supported chassis; this does not enable Clan chassis technology flags.


The Clan Gauss rifle retains the standard 15-damage projectile, inert ammunition
and 20-point mount explosion, with its own six-slot, twelve-ton construction.
Clan normal/light/heavy machine guns use ballistic skill and normal payloads of
two/one/three damage. All three share gatling fire's one-die damage and heat,
three-round expenditure, low-supply behavior and targeting-computer exclusion.
Their individual ranges, mass and ammunition capacities remain catalog facts.


Clan LRM-5/10/15/20, SRM-2/4/6 and StreakSRM-2/4/6 now share missile firing,
ammunition, one-shot supply and saved recycle processing. Clan LRMs have no
minimum range and support hotloading. Their LRM-10/15 cluster tables retain the
reference differences from IS launchers; LRM packets remain capped at five
damage, while SRMs hit individually. Clan Streaks retain ammunition and heat
when no lock is achieved. The inspected catalog assigns Clan LRM-20 the
LRM-10 recycle constant, so its implemented recycle is twenty seconds.


Clan LB2/5/10/20-XAC and UltraAC/2/5/10/20 now use the shared ballistic rules.
LB-X launchers select slug or cluster bins and resolve the established pellet
tables; Ultra autocannons use the same two-round supply, single-round fallback
and permanent loader-failure rules. Each keeps its Clan construction and range
facts. Clan UltraAC/20 supports linked split criticals, as declared by the
reference catalog.


Clan biped chassis now derive their shared technology from the Clan flag.
Heat sinks automatically have double efficiency and occupy paired criticals;
either critical loss removes the whole pair and two units of cooling. All
sections have built-in CASE containment. XL/XXL layouts use Clan side-torso
slot totals, including unambiguous relocated XL installations, while engine damage
still counts actual lost slots. Seven Endo Steel/Ferro Fibrous criticals
activate Clan material savings, using the reference's 60-denominator armor
conversion. These are derived construction facts; no additional persisted
technology cache is introduced.

### Searchlights

On a running unit fitted with `Searchlight`, `slite` starts a five-second on/off switch. Repeating the command preserves its countdown. Lua uses `btech.unit.slite(unit, pilot)`; `btech.unit.state(unit).searchlight` reports hardware and switch state. Switches and damage survive restart.

An intact active lamp illuminates its carrier and targets within 30 hexes in its forward torso arc, subject to terrain, woods and water obstruction. Illumination follows current positions and facing and feeds optical detection and aiming. Front torso hits can destroy the lamp and cancel its switch.

Clan rotary autocannons (`CL.RotaryAC/2`, `/5`, `/10`, `/20`) support the same `rac` burst controls and `unjam` recovery as Inner Sphere rotary mounts. Burst size, remaining ammunition and the attack roll determine ammunition expenditure, heat and jams; each shell that hits applies its weapon's full damage.

Use `mechprefs SLWarn ON` to receive external searchlight entry/exit warnings, `OFF` to silence them, or `mechprefs SLWarn` to toggle. The preference defaults to off and persists with the unit. Lua provides `btech.unit.searchlight_warning(unit, pilot, enabled)`. Multiple beams produce a single illuminated state; warnings resume after restart without repeating an already observed transition.

`mechprefs ArmorWarn` and `mechprefs AmmoWarn` toggle combat warnings; append `ON` or `OFF` to choose explicitly. Both default to on and persist with the unit. Armor warnings report transitions to low, critical or breached protection separately for front and rear armor. Ammunition warnings use installed-bin weighting and the firing mode's warning window, including shots that miss. Lua exposes `btech.unit.armor_warning(unit, pilot, enabled)` and `btech.unit.ammunition_warning(unit, pilot, enabled)`.

`mechprefs FFSafety ON` blocks non-coolant weapon fire at units on your team. Use `OFF` to disable it or omit the setting to toggle; the default is off. A battlefield with the no-friendly-fire flag (256) independently blocks these attacks. Coolant guns can still cool teammates or their own carrier. Lua provides `btech.unit.friendly_fire_safety(unit, pilot, enabled)`.

### Cockpit status

`status` displays the local unit's power, position, movement, heat, armor, internal structure and weapon readiness. Any conscious occupant can inspect it, including while shut down. Select `status armor`, `info`, `weapons`, `heat` or `short`, or combine compact `AIWH` selectors. The short view takes precedence when selected.

Lua uses `btech.unit.status(unit, options)` and receives the same styled text.
Inspection does not advance timers, acquire contacts or consume dice. The current
display has fixed cockpit columns and ASCII silhouettes; see
[the status display contract](btech-status-display.md). `R` selects the full
display with the model name, and is not a separate armor layout.

`status N` emits the compact chassis record; `status NW` adds weapon locations and live ammunition columns. Ammunition groups are paired with rows in encounter order, rather than matched to that row's weapon. Empty groups are omitted. `S` takes precedence over export selectors. Info status also shows jump/fall progress, mechanical movement limits, targeting, selected sensors, recovery and feed-clearing countdowns.


## Biped kicks

Native `kick [left|right] [#unit]` and Lua `btech.unit.kick(unit, pilot, leg, target)` share an atomic Rust resolver. The default leg is right; an omitted target uses the selected target. Explicit targets require an acquired contact. The direct Rust profile API is read-only.

Kicks require a running, standing, unstunned biped, both legs and hips, no outstanding limb recovery, and no recycling weapon in the selected leg. Targets must be within strictly one hex of spatial range and in the real forward arc, with clear terrain LOS and a permitted elevation difference. Unit and map friendly-fire restrictions and the map's no-physical-attacks flag apply.

Aim includes configured piloting, actuator, movement and terrain modifiers. Damage uses nominal tonnage, actuator losses, Melee Specialist and the configured glancing policy. Target posture and elevation select weapon, punch or kick locations. Damage cascades and the subsequent target-on-hit or attacker-on-miss balance check commit together with falls, pilot protection, dice and messages.

Each attempt starts a 60-second recovery timer on the selected leg, including misses. Recovery blocks another physical attack and weapons in that leg, persists through saves, pauses while shut down and clears on startup. Status and Lua state expose the remaining time. Lua callback failure rolls back the entire action and its messages.

Tests cover actuator penalties, rejected-action immutability, explicit miss and glancing boundaries, deterministic replay, native/Lua parity, callback rollback, persisted expiry and shutdown/startup recovery. Other physical attacks, TSM and physical-combat XP remain unfinished.


## Biped punches

Native `punch [left|right|both] [#unit]` and Lua `btech.unit.punch(unit, pilot, arms, target)` use the same Rust physical resolver as kicks. Selection defaults to both arms, attempted left then right. The Rust API exposes `BattlePhysicalRules`, `BattlePhysicalProfile` and `BattlePhysicalReport` for shared mechanics, plus punch selection and batch reports. Attack profiles identify the attack kind and limb; the separate balance report is present for kicks, successful trips and missed mace swings.

Each arm requires its section and shoulder, and all weapons in that arm must have completed recycling. Missing lower-arm or hand actuators impose aim penalties; upper/lower actuator losses also halve damage. Punches use nominal tonnage divided by ten, rounded down, before specialization and actuator adjustments. Unlike kicks, punches do not require both hips and do not perform an extra balance check on either a hit or miss. Damage can still cause normal critical, injury and fall consequences.

Punch arcs include the selected side and front after torso rotation. Arm flipping does not change these physical arcs. Against supported bipeds, a puncher must stand; a target at equal elevation must also stand. A target one level higher uses the kick table if standing or the weapon table if prone; equal-height standing targets use the punch table. Common range, visibility, terrain, friendly-fire and no-physical-attacks restrictions apply.

An accepted punch starts a 60-second arm recovery timer. A two-arm action checks existing physical recovery before starting, then permits its own first arm's timer while attempting the second arm. A disabled, recycling or out-of-arc arm is reported without suppressing the other arm. Later targeting changes, including destruction caused by the first punch, are checked before the second punch. If neither arm can attack, the action returns an error without changing state. Damage-resolution failure or Lua callback abort rolls back the entire action and messages.

Tests cover ordered two-arm attacks, shoulder/weapon rejection with the other arm continuing, miss/glancing boundaries without extra balance dice, torso and side arcs, flipped arms, elevation tables, native/Lua message parity, rollback and saved recovery/dice replay. References: `combat/mech_physical.c`, `combat/mech_physical_resolution.c`, `combat/mech_physical_damage.c`, and `unit/mech_identity.c`. Other unit classes, carried clubs, physical weapons, charges/DFA and physical-combat XP remain open.


## Biped trips

Native `trip [left|right] [#unit]` and Lua `btech.unit.trip(unit, pilot, leg, target)` share the physical attack resolver. The leg defaults to right; omitting the target uses the current selection. Rust exposes `battle_trip_profile`, `resolve_battle_trip` and the shared `BattleLeg`, physical rules and report types.

A trip requires both legs and hips and follows kick range, elevation, real-forward-arc and recovery restrictions. Targets must be standing and not rising. Physical aim uses the configured kick base, movement, terrain and specialization; unlike kicks, trips receive no upper/lower/foot actuator aim penalties.

Trips apply zero direct impact damage and consume no hit-location roll. A successful attack forces the target to make a normal piloting check. Failure produces a one-level fall with normal damage, pilot protection and terrain consequences; success leaves the target upright. Glancing mode affects the attack threshold and notices without reducing the balance check. A missed trip requires no attacker balance roll. Every accepted attempt starts the selected leg's 60-second physical recovery timer.

The complete attempt, dice, fall cascade, recovery timer and notices are transactional. Tests verify missing actuator penalties, hip guards, prone/rising target rejection, exact dice consumption on hits and misses, glancing boundaries, target falls, native/Lua message parity, callback rollback and saved recovery. References: `combat/mech_physical_commands.c`, `combat/mech_physical_resolution.c`, and `combat/mech_physical_damage.c`. Quad trips and physical-combat XP remain unfinished.


Cockpit status returns explicit styled-text source through Rust and Lua. Native `status` uses the styled report path, so surplus-ammunition colors honor each recipient's color setting. Unit names, compact-export identity fields and weapon annotations are escaped as literal text before rendering. Bracket-like text in a name and markers such as `[OS]` remain visible; they do not change the report's style. Scripts may pass `btech.unit.status` directly to styled notification APIs, or use `text.strip` to obtain its visible text. Rendering tests cover plain and ANSI output, literal fields, ammunition color thresholds and native/Lua source parity without world mutation.


## Triple Strength Myomer

`TripleStrengthMyomer` is a supported passive critical-slot system. Six or more installed slots enable the technology, including templates that omit `TripleMyomerTech`; an explicit technology flag requires those slots. Partial installations are rejected during construction. The slots add no mass and cannot receive random critical hits. Explicit slot loss, section loss or flooding does not disable the installed technology.

Activation is derived from the last sampled excess heat: at least nine enables TSM. Stored weapon heat alone does not enable it, and no extra activation timer or mutable flag is saved. Kicks and punches double their base tonnage damage before Melee Specialist and actuator halving; trips remain non-damaging.

Mechanical `mobility.maximum_speed` remains damage-only. `movement_maximum_speed` is the current throttle ceiling and includes the rounded TSM walking-MP bonus. Speed and movement-heat updates apply the reference's second walking-MP conversion, while turning adds 1.5 MP to the effective ceiling. Attacker movement aim uses the template maximum plus 1.5 MP. These are deliberately separate calculations. For a 118.25-kph Jenner, active TSM produces a 129-kph throttle ceiling, a 150.5-kph update ceiling and a 145.125-kph turning ceiling.

At heat nine, the five-heat movement penalty is suppressed. At heat ten and above, normal heat penalties apply using the update ceiling. Acceleration, native `speed run/walk/back`, Lua speed controls, walking restrictions, stun, stand timing and movement-heat production use their corresponding effective limits. Damage that removes all mobility cannot gain motion from TSM.

Cooling may leave actual speed and desired throttle above the cold ceiling until a movement update. Saved validation permits that bounded hot-motion state; the forward throttle is clamped during the next update and speed changes through normal acceleration. The update uses the previously sampled throttle for that tick, and reverse throttle retains the reference's asymmetric clamp behavior. Status shows activation and the current throttle maximum; Lua unit state exposes `triple_myomer_active` and `movement_maximum_speed`.

Tests cover installation counts, implicit/explicit technology, mass, critical eligibility, passive losses, heat thresholds, physical damage rounding, native/Lua speed parity, movement, turning, heat production and cooling/restart transitions. References: `unit/template_specials.c`, `combat/crit_dispatch.c`, `combat/mech_physical_damage.c`, `combat/mech_bth_movement.c`, and movement files `mech_move.c`, `mech_update_speed.c`, `mech_update_motion.c`, `mech_update_heat.c`. Cargo/towing, sprinting/boosters, other movement classes and their TSM interactions remain part of their unfinished gates.


## Axes and swords

Native `axe [left|right|both] [#unit]` and `sword [left|right|both] [#unit]`, and Lua `btech.unit.axe` / `btech.unit.sword`, use the shared physical resolver. Omitted arm selection considers both arms, left first, skipping arms without enough operational weapon parts. Explicit arm selection reports missing equipment. Both require a surviving arm, shoulder and hand, with no recycling ranged weapon in that arm.

The shared Rust API uses `BattleArm`, `BattleArmSelection`, `BattleArmAttack` and `BattleArmAttackReport`; `battle_arm_attack_profile` is read-only and `resolve_battle_arm_attack` commits the action. Punches still permit two arms in one action. An accepted axe or sword swing starts 60-second recovery, which blocks another swing with the second arm; an unusable first arm can leave the second arm available. Expected arm rejections preserve earlier accepted attacks, while impact errors and callback aborts roll back the entire action and all staged messages.

Axes require at least `floor(tons / 15)` operational same-arm `Axe` slots. Swords require `floor((tons + 15) / 20)` operational same-arm `Sword` slots. These observable attack thresholds differ from mass accounting. Each axe slot weighs 1,024 mass units; each sword slot weighs `floor(ceil(tons / 10) * 512 / ceil(tons / 15))` units. Lost criticals retain mass until their section is lost. Both systems are eligible for random critical hits and emit their equipment-loss messages; remaining usable parts determine whether another attack is possible.

Fixed base aim is four for axes and three for swords. With pilot-skill physicals enabled, axes use piloting minus one and swords piloting minus two. Upper/lower actuator failures add two aim points each, but do not halve hand-weapon damage. A missing or failed hand prevents the attack. Axe base damage is `floor(tons / 5)`; sword base damage is `floor((tons + 5) / 10) + 1`. TSM doubles that base before Melee Specialist, and glancing damage rounds upward after halving. Neither attack adds a separate balance check.

Arm arcs include torso rotation and the selected side. Equal-elevation standing targets use the weapon hit table; lower standing targets use punch locations, higher standing targets kick locations, and higher prone targets weapon locations. Prone targets at equal or lower elevation cannot be attacked. Common range, terrain, airborne, friendly-fire and map restrictions still apply.

Tests cover part availability, mass rounding, actuator/hand loss, TSM, glancing damage, critical feedback, elevation tables, default arm selection, accepted-hit/miss recovery, native/Lua messages, callback rollback and restart. References: `combat/mech_physical.c`, `combat/mech_physical_commands.c`, `combat/mech_physical_resolution.c`, `combat/mech_physical_damage.c`, `combat/crit_mechs.c`, and `unit/mech_consistency.c`. Clubs, claws, saws, charge/DFA and physical-combat XP remain unfinished.


## Maces

Native `mace [left|right|both] [#unit]` and Lua `btech.unit.mace` use the shared arm-weapon resolver. A mace requires `floor(tons / 10)` operational Mace slots in its arm, together with an operational shoulder and hand. Each slot adds 1,024 mass units and is eligible for critical loss. Losing enough parts disables subsequent swings.

Fixed base aim is four; pilot-skill aim uses piloting minus one. Maces add a separate +2 weapon aim modifier to either base. Base damage is `floor(tons / 4)`, doubled by active TSM before specialization and glancing adjustments. Arm actuator damage affects aim but does not halve mace damage. Targeting, elevation tables, default arm selection and sixty-second recovery follow axes and swords.

A missed swing requires attacker piloting at +2. Failure causes a one-level fall; success leaves the attacker standing. A hit has no additional balance check beyond normal damage consequences. The shared transaction contains dice, falls, recovery and notices. Tests cover both balance outcomes as well as construction, critical loss, native/Lua parity, rollback and restart. References: `combat/mech_physical.c`, `combat/mech_physical_resolution.c`, `combat/mech_physical_damage.c`, `combat/crit_mechs.c`, and `unit/mech_consistency.c`. Other physical weapons, charge/DFA and physical-combat XP remain open.


## Dual saws

Native `saw [left|right|both] [#unit]` and Lua `btech.unit.saw` share the physical arm transaction. Seven operational `Dual_Saw` slots in one arm are required. Each part weighs 1,024 mass units and can receive a critical hit. A saw needs a surviving arm and shoulder, but no hand actuator. Upper/lower actuator failures each add two aim points without reducing damage.

Base aim is four, or piloting minus one with pilot-skill physicals, plus a separate +1 saw modifier. Base damage is always seven, without a TSM boost. Melee Specialist adds one before glancing damage is halved and rounded up. No additional balance check occurs on a hit or miss. The first accepted arm starts sixty-second recovery and blocks the second arm.

The current reference damage switch leaves saw location at section zero: the left arm on a biped. It consumes no hit-location dice and applies front damage without a through-armor critical flag. The Rust profile exposes `fixed_location` for this behavior; its normal `hit_table` is unused when a fixed location is present. Normal damage cascades still apply. This observable behavior is preserved explicitly rather than silently assigning an axe or punch table.

Tests cover seven-slot mass and availability, critical-loss notices, hand/actuator loss, fixed location and dice, TSM immunity, glancing rounding, native/Lua parity, callback rollback and persisted recovery. References: `combat/mech_physical.c`, `combat/mech_physical_resolution.c`, `combat/mech_physical_damage.c`, `combat/crit_mechs.c`, `unit/equipment_types.h`, and `unit/mech_consistency.c`. Claws, clubs, charge/DFA, other classes and physical-combat XP remain open.


## Claws

Native `claw [left|right|both] [#unit]` and Lua `btech.unit.claw` use the shared arm-attack transaction. Each arm requires `floor(tons / 15)` operational `Claw` slots. Each slot weighs 1,024 mass units and is eligible for critical loss. Lost parts retain mass; the surviving-part count determines availability. The reference does not require an operational shoulder or hand for claws.

Fixed base aim is four, or unadjusted piloting with pilot-skill physicals enabled. A separate +1 claw modifier applies. Failed upper/lower arm actuators each add two aim points without halving damage. Damage is `floor(tons / 7)`, doubled by active TSM before specialization and glancing rounding. Like saws, the current reference applies front left-arm damage without a hit-location roll; the Rust profile represents that with `fixed_location`. Normal damage consequences still consume their own dice. Hits and misses have no additional balance check.

Default selection tries usable arms left then right. Like punches, a claw action permits the recovery started by its own first arm while attempting the second arm. Each accepted swing starts sixty-second recovery. Expected per-arm rejections can leave the other arm available; a transactional failure rolls back the entire action.

Tests cover part thresholds, mass, failed shoulder/hand and arm actuators, pilot aim, TSM/glancing damage, direct-location damage and dice, default selection, native/Lua two-arm parity, rollback and saved recovery. References: `combat/mech_physical.c`, `combat/mech_physical_resolution.c`, `combat/mech_physical_damage.c`, and `unit/mech_consistency.c`. Clubs, charge/DFA, other unit classes and physical-combat XP remain open.


## Trees and two-handed clubs

Native `grabclub [left|right|-]` and Lua `btech.unit.grabclub` acquire a tree from light or heavy forest, or release it with `-`. Default selection uses the first usable arm, left before right. Acquisition requires a standing, powered biped, no jump/free fall or active unjamming, a working arm/shoulder/hand and no recycling ranged weapon in that arm. Usable axes, swords or maces prevent acquisition. It consumes neither terrain nor dice and starts sixty-second recovery in the carrying arm. Existing physical recovery does not prevent acquisition.

`carried_club` is one optional arm in owned unit state, exposed through Rust, Lua state and cockpit status. It survives restart and blocks punching with that arm. Explicit release, shutdown, destruction and loss of the carrying hand clear the state; explicit release, shutdown and tactical hand loss deliver shattering notices. Normal falls alone do not discard the tree.

Native `club [#unit]` and Lua `btech.unit.club` use a carried tree, or an immediate tree when standing in forest. Both arms, shoulders and hands must work, and both arms' ranged weapons must be ready. The attack uses the torso-adjusted forward arc, normal physical reach and axe-like elevation/location rules. Base aim is four, or piloting minus one; each upper/lower actuator failure adds two. The reference checks installed actuator types in the right arm for both arms' penalties; Rust preserves this distinction from operational loss. Base damage is `floor(tons / 5)`, doubled by TSM before specialization and glancing rounding, without actuator damage halving.

Every accepted swing recycles both arms for sixty seconds. A hit shatters the carried tree before applying damage; a miss keeps it. Forest-supplied immediate clubs need no additional saved state. Neither outcome adds a separate balance check. Native/Lua transactions include acquisition, recovery, tree breakage, damage, dice and staged notices. Tests cover acquisition guards, hand loss, shutdown, punch restrictions, damage, breakage, two-arm recovery, native/Lua parity, callback rollback and saved carrying state.

References: `combat/mech_club.c`, `combat/mech_physical.c`, `combat/mech_physical_resolution.c`, `combat/mech_physical_damage.c`, `combat/crit_mechs.c`, and `unit/mech_lifecycle.c`. Charge/DFA, physical-combat XP and unsupported unit classes remain unfinished. Pod-removal interactions belong to the pending iNARC implementation.

## One-way charge collision resolver

Rust exposes `battle_charge_profile` and `resolve_battle_charge` with `BattleChargeRules`. This is the collision component for the pending movement integration; native/Lua charge initiation, accumulated-distance tracking, timeout and mutual-charge dispatch are not yet delivered.

The resolver accepts a pair of live tactical bipeds within the strict 0.6-hex collision trigger. The attacker must be powered, moving forward at least one MP and have no physical recovery. Jumping/free-falling participants and prone targets are rejected. The front arc uses real heading, ignoring torso rotation. The one-way reference checks ranged-weapon recovery in native section indices zero through five: both arms, both side torsos, center torso and left leg. This differs from the six sections that receive recovery.

Damage uses integer actual tons from current construction mass, not nominal design tons. The normal rules use current velocity; new rules use accumulated distance times one MP. Relative speed subtracts the opponent's velocity projected onto the integer heading difference. Single-precision arithmetic and truncation toward zero determine the result. Inflicted damage uses `(mass + 5) / 10` with a final one-point bonus, plus Melee Specialist. Standard recoil uses the opponent's `(mass + 5) / 10`; new level-three recoil uses relative speed and divisor twenty. A nonpositive inflicted result aborts without consuming dice.

Aim starts at five, adds attacker minus target piloting, attacker movement (including the Melee Specialist adjustment) and target movement. It does not use ordinary physical-attack immobility or terrain modifiers. Aim above twelve aborts before the roll. A miss consumes the attack roll and starts recovery without damage, balance checks or stopping motion.

On a hit, independently located packets of up to five damage strike the target, followed by recoil packets against the attacker. Normal tactical damage cascades remain inside the enclosing world checkpoint. Surviving participants check piloting at +2, attacker before target; failed checks cause a one-level fall. The attacker then stops. Both outcomes start sixty-second recovery in both arms, both legs and both side torsos. Those section timers use the existing persistence and expiry machinery.

Tests cover relative velocity/headings and truncation, configured damage modes, actual mass, rejected attempts, seeded hit/miss behavior, packet totals, control modifiers and saved six-section recovery. References: `combat/mech_charge.c`, `combat/mech_physical_damage.c`, `movement/mech_charge_tracking.c`, `unit/mech_identity.c` and `unit/mech_specification_state.c`. Mutual charges, initiation/cancellation, movement tracking, timeout and full end-to-end charge parity remain open; this component does not close that gate.


Charge recoil is now sampled after the target's damage cascade. The profile remains the pre-collision forecast; `BattleChargeReport.received_damage` and `recoil_arc` describe recoil actually applied, with zero/absent values on a miss. New level-three recoil uses the target's post-damage speed and the participants' current headings, while retaining the collision's sampled tonnage. Recoil direction also uses the post-damage positions. This follows the reference's phase ordering and cached collision weight without introducing a mutable mass cache. A seeded test destroys a moving target's leg during the first damage group, verifies its stop, and checks the resulting increase in recoil. Mutual-charge integration remains pending.


## Mutual charge collisions

Rust `resolve_battle_mutual_charge(world, first, second, rules, second_distance)` commits an opposing pair as one transaction. Both eligibility decisions precede damage. If at least one participant can attack, both units consume an attack roll before either collision resolves, including a rejected participant. Both eligible participants receive their roll messages before either impact message. The second attempt retains its eligibility and roll after damage from the first, even if it is knocked down. If neither qualifies, neither consumes dice or receives recovery. Per-participant rejection reasons are returned with the combined notices.

Mutual attacks use their six charge sections for ranged recovery, and test unconsciousness and stun. The first attack uses the real forward arc; the second uses its torso-adjusted weapon arc. The first inflicted damage uses relative speed without the one-way +1; the second inflicted damage uses its own (actual tons + 5) / 10. Specialization applies to each. Level-three first recoil uses the first unit's mass, while second recoil uses its opponent's. The second recoil uses calculated damage for full five-point groups but its inflicted-damage remainder for the final group, preserving the current reference behavior.

Each successful collision completes its damage and recoil, stops its attacker, and checks the original first participant's piloting before the original second participant's. Only then does the second collision run. Both participants receive six-section recovery after the pair if either attempt was accepted, regardless of hit or eligibility. The movement owner will clear both charge selections; selection tracking and automatic dispatch are still pending.

The torso merge can retain both left and right flags. `BattleTorso::Both` represents that state explicitly, and shared geometry uses its rightward 59-degree offset. Left/right control requests from that state are rejected; centering clears it. Both is an inspected/persisted result, not an accepted rotation direction. Real-forward charge probing restores left first before merging the other pose, matching the reference's ordering. Tests verify the merged pose, geometry and control behavior.

Seeded tests cover hit/miss pairs, rejected participants, no accepted attempts, recoil remainder behavior, a second attack after first-impact leg destruction, and restart/recovery. References are `combat/mech_charge.c`, `unit/mech_condition_state.c`, `unit/mech_identity.c`, and `movement/mech_move_controls.c`. Blindness, unsupported unit classes, native/Lua charge controls, movement tracking/timeouts and full end-to-end parity remain in their unfinished gates.


### Charge selection and ground movement

`charge` selects the current target lock, `charge #unit` selects an acquired visible unit, and `charge -` cancels. The Lua equivalent is `btech.unit.charge(unit, pilot, target)`, with nil for the current lock and `"-"` to cancel. Selection does not initiate motion or consume dice. Unit state and cockpit status expose the selected target and accumulated counters.

The ground movement owner supplies configured `newcharge`, `tl3_charge`, `extendedmovemod` and `hit_arcs` policies to the collision component. With new charge rules, active movement updates increment the timer and integrated travel increases distance before terrain consequences. The post-increment timeout expires on update 61. Stationary selections do not schedule movement; turning can age the timer but cannot trigger impact at zero speed. Endpoint range must be strictly below 0.6. An attempted collision resets selection and counters; mutual dispatch resets both participants so another movement update cannot dispatch the same pair again. Damage errors roll back the enclosing movement tick, including counters and dice.

Selecting a different target preserves counters. Explicit cancellation and timeout reset everything; shutdown only clears the target. Persistence owns these fields in the Rust unit model. No bridge or C build dependency is introduced. Airborne tracking and eligibility for movement modes not yet represented in Rust remain separate coverage gates.


### Charge tracking during jumps

Ground and jump updates now share `BattleMovementRules`. Active jump updates age new-rule charge selections without adding travel distance; stabilization and free-fall events do not run the charge timer. Integrated airborne endpoints check collision range and reject airborne attacks without spending attack dice or applying recovery. Mutual airborne rejection clears both selections. Landing and terrain-entry consequences precede the endpoint check, so normal landing uses grounded eligibility. Bridge interruption and lost thrust retain their early exit before endpoint collision dispatch.

Focused tests verify timer expiration and persisted flight replay, old-rule counter preservation, unchanged charge distance, one-way/mutual airborne rejection without combat dice or damage, landing-before-eligibility order, and inactive tracking during stabilization. This closes the conventional jump-tracking gap noted above. Unimplemented charge eligibility modes, blindness, DFA, physical XP and other unit classes remain open.


### DFA landing calculation component

The read-only Rust `battle_dfa_profile` now characterizes biped landing eligibility and damage forecasts. The caller retains airborne state until after calculating aim. Eligibility requires the same map and occupied hex; it checks ranged recovery in arms, legs and side torsos, and physical recovery in arms/legs only. Airborne targets and map-prohibited friendly fire reject the attack. Fixed base aim is five; pilot-based aim uses the pilot target directly. Melee specialization changes attacker movement to `min(0, movement) - 1`; target motion includes immobility and close-range prone adjustments. Aim above twelve declines without mutation or dice.

Impact damage uses floor(3 * actual whole tons / 10), plus one when nominal tonnage has a remainder modulo ten, plus specialization. Recoil uses nominal tons / 5. Initial target hits use the punch table when standing and the weapon table when prone. The eventual resolver must resample posture for each packet, because earlier damage can cause a fall.

Verified coverage includes mass/nominal-rounding separation, airborne aim, specialization, immobile/prone targets, limb versus weapon recovery, moved targets, configured pilot aim, and rejection without changing state. DFA damage resolution, landing integration, targeting commands and transactional replay remain unfinished.

Reference follow-up: `mech_bth_movement.c::mech_target_movement_modifier` includes a -4 immobility adjustment. The DFA component and ordinary physical attacks account for represented immobility; the charge component currently uses only target speed and needs this correction with updated characterization tests. Blindness and fortification remain unrepresented eligibility states.


### Charge target immobility correction

Charge, DFA forecasts and ordinary grounded physical attacks now share the close-range target movement calculation. In addition to speed, shut-down or unconscious targets receive -4 aim; prone biped targets receive -2 where attack eligibility permits them. Charge still rejects prone targets before calculation. This corrects the missing charge adjustment identified during DFA characterization. Blindness and fortification remain unrepresented states.

Tests explicitly compare the same roll against operational and shut-down targets in both one-way and mutual collisions, and verify unconscious-pilot aim without changing damage or consuming profile dice. The existing miss/recovery and torso-merge fixtures now use operational targets so they continue to exercise actual misses. Tests for all existing physical attacks and DFA use the shared calculation without changing their rules.


### DFA damage resolution

`resolve_battle_dfa` now resolves an accepted biped landing attack in a single world candidate. It calculates aim before ending airborne state, draws the attack roll, and applies target damage followed by recoil in groups of at most five. Standing targets use punch locations; each later packet resamples posture so damage-triggered falls switch to weapon locations. Hit recoil uses front kick locations. If recoil leaves the attacker standing, attacker +4 and target +2 control checks follow in that order, each with ordinary fall consequences on failure.

Misses use rear weapon locations for nominal-tonnage recoil, then a +2 pilot-injury check and explicit prone state. They do not call the ordinary fall resolver merely to become prone, avoiding duplicate damage. Terrain elevation and flooding are applied, including sub-surface ice elevation. Both outcomes apply six recovery timers. Reports retain ordered impact cascades, checks, injuries and notices; a late failure discards the entire candidate.

The shared impact engine now distinguishes admission of a new attack from continuation of an accepted packet sequence. DFA and charge packets continue after an earlier packet destroys the unit, preserving subsequent location/damage processing. Starting a new direct attack on a destroyed unit still fails.

Tests cover hit/miss damage distributions, control ordering, injury, replay, saved recovery, target knockdown changing subsequent locations, continuation after a torso kill, late rollback and ice immersion. DFA targeting commands, durable jump intent, landing dispatch, complete landing interaction parity and physical XP remain unfinished. This component does not yet make targeted jumps available to players.


### Durable targeted-jump launch state

The Rust `launch_battle_dfa` API selects an explicit acquired target or the current target lock and fixes its destination hex at launch. Both forms require a visible live tactical target on the same battlefield and use normal jump authorization and route checks. Target range is sampled at launch for capacity and apex; the snapped destination center remains independent of that range. Saved targeted paths preserve these inputs and reject contradictory projection/target-range admission modes.

The flight cursor owns the optional DFA target identity, so progression and save/load retain it while shutdown, destruction or aborted flight remove it with the flight. Subsequent target motion does not steer the path. Flight validation rejects self-targeting but does not require a vanished target to remain present; landing must handle that case.

Tests verify launch-time range versus snapped distance, fixed destination under target movement, persisted flight replay, selection rejection without mutation, shutdown cleanup and stored-path validation. Lua state type annotations include the new fields. Targeted-jump landing dispatch and native/Lua command exposure remain pending: this launch API is a component for that integration, not a completed player-facing DFA action.


### DFA landing dispatch and commands

Native `jump #unit` and argument-free `jump` now select a DFA target; coordinate jumps retain their existing syntax. Lua uses `btech.unit.dfa(unit, pilot, target)`, with nil for the current lock. Command and Lua selection share the launch transaction and rollback staged output together. Status exposes the flight's DFA target.

Normal and pilot-requested early landings share `BattleMovementRules`, including configured physical pilot skill, movement modifiers, hit arcs and fall rules. Landing inspects DFA eligibility while the flight still supplies the airborne aim modifier. An accepted attack resolves once, clears flight intent, and suppresses ordinary completion/crowding messages and the twelve-second jump stabilization timer. Moved or ineligible targets fall back to normal landing. Subsequent damaged-gear checks and flooding still run. Completed flight cursors are permitted only inside a landing transaction; world validation prevents committing an unresolved arrival.

Tests cover fixed-base airborne aim at actual arrival, saved replay, one-shot dispatch, normal fallback without attack dice, native/Lua default and explicit selection with rollback, and early-landing hits under pilot-based rules. Existing movement, physical and landing suites remain regression gates. Broader landing interactions (including full stagger behavior), unsupported unit classes, special movement/eligibility states and physical XP still require further characterization; this does not claim complete BattleTech parity.


### Jump stagger and experience dependencies

Reference inspection distinguishes `rd.stagger_damage` from both rolling damage history and traditional turn damage. `mech_condition_summary.staggering` and jump/landing modifiers read that separate signed counter. Ordinary damage appends history or turn damage; the counter is reset to zero, set to -10 by the older event handler, or restored from a snapshot. Consequently the Rust rolling history must not be substituted as a trigger for the separate jump-specific check. Restored nonzero-counter semantics remain a distinct compatibility/characterization question.

Physical damage experience is restricted to in-character participants and active pilots in the reference, so tactical simulation attacks must not start awarding it. In-character casualty/action integration remains unfinished.

The shared Rust experience component now calculates awards under explicit catalog policy: strict thirty-second intervals, continuous-XP and explicit-override exemptions, low-24-bit balance, raw-skill-based thresholds scaled by factors of three, and strict `balance > threshold` earned-level boundaries. Recalculation retains total balance rather than spending it, and stored earned bonuses do not feed back into the raw target. Arithmetic is bounded for large inputs instead of relying on signed overflow. `award_battle_character_experience` updates named character values atomically; game-action eligibility and catalog lookup remain the caller's responsibility. Tests cover timing boundaries, wrapping, cumulative thresholds, zero-threshold behavior, extreme values, unchanged state on rejection, and saved/reloaded awards. Combat XP hooks are not enabled by this foundation.

### Skill catalog and named experience awards

`BATTLE_SKILLS` defines canonical skill names, attribute categories, default XP thresholds and continuous-award flags. `battle_skill_definition` accepts full names or generated short names, case-insensitively. Full names take priority; shared aliases select the first catalog entry (`GunBat` selects Gunnery-Battlemech and `TecBat` selects Technician-Battlemech). Use full names to address other skills with colliding aliases.

`award_battle_skill_experience` stores awards under the canonical name and applies current runtime thresholds and catalog timing policy. Action authority and eligibility remain the caller's responsibility. Explicit threshold policies remain available through `award_battle_character_experience`; combat award hooks are not yet connected. Lua's read-only `btech.character.skills()` returns a detached array of catalog metadata. `btech.character.list('skills', player)` returns canonical names with a nonzero saved level or XP; omit the player to list every skill. Player IDs, names, account aliases and #dbrefs are accepted. The `advantages` and `attributes` categories always return their complete lists, matching the reference's filtering condition. Full category names are case-insensitive; abbreviations, explicit nil and invalid targets are rejected. This query requires a callback and does not mutate character state. Catalog membership does not imply implementation of every associated game action.

Wizard command `@btech skill-threshold <skill>[=<value>]` and Lua `btech.character.set_threshold(player, skill, value)` update runtime thresholds from zero through 2147483647. `btech.character.threshold(skill)` reads the effective threshold; `skills()` continues to expose catalog defaults. Updates are canonicalized, checkpointed and rolled back with failed callbacks. Zero disables earned levels on the next accepted award; changing a threshold does not immediately rewrite character XP. Database reload restores catalog defaults while retaining awarded XP.

`battle_skill_progress(world, player, skill)` and Lua `btech.character.progress(player, skill)` inspect progression without mutation. Results include the canonical name, raw/current skill targets, stored earned levels, XP balance, runtime threshold, total balance required for the next stored level, and additional points remaining. The total includes the strict threshold boundary: at base target four and threshold 3000, successive totals are 3001, 12001 and 39001. Disabled progression has no next total. Threshold changes can leave stored levels behind until another accepted award; inspection preserves that state and reports zero remaining when the next stored level is already affordable. Large theoretical totals saturate at `u64::MAX`; the saved balance itself remains limited to 24 bits.

`retain_battle_character_experience(world, player, per_mille)` applies proportional XP retention atomically to saved skills, advantages and Lives, matching the reference state adapter. For example, 500 retains half the balance, rounded down; zero clears XP, and 1000 retains the balance while recalculating earned skill levels. Current runtime skill thresholds determine the new bonuses; advantages and Lives use their zero default thresholds. Signed-negative XP bit patterns clear to zero. Base values and last-use timestamps remain unchanged. Other saved entries remain untouched and do not block evacuation. The shared casualty evacuation caller applies the configured retention after successful movement, exempting wizards and restoring movement, XP and output together on failure.

Lua `btech.unit.evacuate(unit, wizard)` moves non-wizard contents of an in-character unit to `battletech.afterlife_dbref`, using ordinary teleport policies and callbacks. Successful moves release cockpit assignments through the usual movement reconciliation. When in-character rules are enabled and `xploss < 1000`, existing character profiles retain that per-thousand fraction of XP. Wizards stay aboard; tactical units do nothing. Occupants are snapshotted before callbacks, so newly arrived objects are not swept into an ongoing evacuation. A denied/redirected move or callback/XP error restores the operation's world changes and staged effects. This callable evacuation path does not yet enable automatic casualties from in-character combat.

`BattleImpactReport::crew_casualty()` identifies lethal head or cockpit damage for conventional bipeds. Ordinary center-torso or engine destruction is not sufficient to infer crew death. The query reads actual damage phases and critical losses and does not move occupants; the enclosing action must apply in-character policy and publish evacuation. Other casualty causes, including fatal character injuries and environmental events, remain separate.

`resolve_battle_impact_action(scripts, config, unit, hit, damage)` combines the material resolver with lethal head/cockpit evacuation. It checkpoints world state and staged effects before damage, invokes ordinary evacuation callbacks for lethal in-character outcomes, validates the resulting world, and restores the entire action if any step fails. Ordinary CT destruction leaves occupants aboard. This host API is not a firing command: callers still own authority, hit selection and nonfatal injury/fall effects. Normal firing and physical attack eligibility are unchanged.

`injure_battle_character_pilot` applies build-scaled health damage to a present pilot in an in-character unit. Nonfatal injuries use the player's existing consciousness dice and recovery state, including Pain Resistance and the caller's effective Toughness rule. Further injury while unconscious does not consume dice or restart recovery. Character-mode injury counts are stored separately from tactical injuries, so six points do not falsely destroy an RPG unit. Fatal health damage records unit loss, releases the pilot and clears the recovery countdown. `injure_battle_character_pilot_action` additionally evacuates occupants in the same checkpoint, restoring health, unit state, dice, movement and XP if evacuation fails. The final fatal injury does not increment the reference's surviving-injury counter. Normal combat callers still need to select this path when processing character injuries.

The impact action now selects ordered character-injury processing for in-character units. Head injuries occur before their material phase; non-hotloaded ammunition/weapon explosion injuries occur after explosion damage if the pilot and unit survive. Pain Resistance reduces explosion injury, and Toughness applies to consciousness checks. Applied injuries appear in `BattleImpactReport.character_injuries` and are removed from pending effects, preventing duplicate application. Fatal health damage is a distinct `CharacterInjury` casualty cause and triggers the same atomic evacuation. Pure material resolution remains available without implicitly changing character health.

Character impact actions apply crew stun through the shared ten-second stun implementation and publish its cockpit notice in the same transaction. Handled stun is removed from pending effects. Character injury actions also publish consciousness attempts, target/roll details and loss-of-consciousness feedback directly to the affected pilot, without exposing health-roll messages to other occupants or consuming additional dice. Failed actions restore staged notices along with world state.

`flood_battle_unit_action(scripts, config, unit, rules)` applies immersion flooding, publishes notices and evacuates in-character occupants when a cockpit becomes newly flooded. Failed movement or callbacks restore flooding, unit loss, movement, XP and staged notices together. Repeat calls do not evacuate again for the same breach. Flooded-leg cascades use the character-capable fall resolver within this action. Ordinary surface-break callers retain the character guard; explicit host actions own terrain and casualty publication.

Ordinary flooding entry points retain the character-mode guard. Character-enabled flooding is confined to the host action until movement and damage callers can publish evacuation in their own transactions.

`fall_battle_unit_action(scripts, config, unit, levels, rules)` resolves character-enabled dry-ground and water falls with the shared posture, direction, damage and flooding logic. Pilot protection failures produce `BattleFallReport.character_injury`; tactical falls retain `pilot_injury`. Grouped damage applies character health at its actual event positions. Nested flooding can create new cockpit casualties, and the host action publishes all newly lethal crew states under one checkpoint. Private injury feedback traverses nested reports once. Ordinary fall entry points retain their character gate; surface-break/airborne interactions and normal combat/movement callers still require integration.

`break_battle_surface_action(scripts, config, map, coordinate, terrain, rules)` breaks ice or a bridge inside one host checkpoint, supporting mixed tactical and character occupants. Character-mode falls also propagate their publication mode to landing-induced ice breaks. Neighbor/trigger order and terrain-before-fall semantics remain shared with ordinary breakage. Terrain, all affected units, character health, evacuation and staged notices roll back together on failure. Ordinary break APIs and upward-breakout/movement callers retain their existing character restrictions pending integration.

`break_battle_ice_upward_action(scripts, config, map, coordinate, unit, rules)` preserves the breaker at its current altitude while resolving neighboring falls, character health and evacuation under one checkpoint. Callers own authorization and the movement that reached the ice plane. Ordinary jump advancement is not yet switched to this host action.

`advance_battle_jumps_action(scripts, config, movement_rules)` is now used by the server heartbeat. It checkpoints airborne advancement, publishes cockpit and private fall feedback, and evacuates newly lethal crews before the enclosing tick commits. Accelerated free-fall impacts and direct lost-thrust/obstacle/bridge falls can resolve character health; ordinary simulation calls retain their tactical guards. Failed evacuation restores the descent cursor and all affected state. Normal character jump launch, landing, stacking and other movement branches remain gated pending integration.

The airborne host action now carries its character mode through landing protection falls, landing-induced ice breaks, vertical ice crossings and immersion. Neighbor falls retain their private injury reports, and newly lethal crews are evacuated before commit. The shared ordinary landing helper still uses tactical mode for callers without casualty publication. Stacking, DFA/charge collisions and interrupted ground movement remain integration gates; character jump launch is still disabled.

`resolve_battle_stacking_action` resolves crowding under a host checkpoint, including character hit injuries, avoidance falls and newly lethal crew evacuation. The airborne action uses the same character-capable resolver during landing and obstacle falls. Collision reports retain direct injuries and nested balance/flooding falls for private feedback; ordinary ground movement still uses the tactical resolver. Character jump launch remains gated while interrupted ground motion and physical DFA/charge paths are integrated.

Interrupted horizontal movement after a descending ice break now propagates the airborne action capability into ground-segment cliff/reverse falls and water entry. Applied falls and flooding retain their nested character reports for host publication, while preserving the altitude already selected by the jump fall. Ordinary ground ticks still use the tactical path. Physical DFA/charge publication remains a gate for character jump launch.

`advance_battle_motion_action(scripts, config, rules)` now drives the server ground tick. Ground and airborne updates share a movement report and host checkpoint/publication helper, retaining fall and stacking injury reports until crew consequences are published. Ground water/ice, cliff/reverse falls and stacking select character processing in the host action; ordinary simulation APIs keep tactical behavior. Physical charge and DFA publication, normal firing and character jump launch remain integration work.

`resolve_battle_physical_attack_action` executes a single typed physical attack with normal cockpit authority and targeting/equipment checks, enabling character impact and balance-fall resolution. It publishes direct and nested private injuries, and evacuates newly lethal crews under one checkpoint. The ordinary physical APIs remain tactical; native/Lua routing, ordered multi-arm actions, physical XP and charge/DFA integration remain unfinished.

`resolve_battle_arm_attack_action` accepts a `BattleArmAttackChoice` and sequences selected arms with character-aware targeting, impacts and falls. Per-arm targeting rejection preserves prior successful attacks, while resolution or casualty-publication failure rolls back the entire sequence. The host publishes aggregate cockpit notices once and traverses each completed attack for private injury feedback. Native/Lua routing and physical XP remain separate integration work.

Native and Lua kick, trip, punch, axe, sword, mace, saw, claw and club adapters now use the shared character-aware host actions. Target/contact checks and configured rules remain shared. The host owns notice publication, preventing duplicate adapter output; Lua keeps its outer transaction so a later callback error restores damage, recovery, crew movement and staged messages. Physical XP and charge/DFA integration remain incomplete.

Direct single and arm-sequenced physical hits now award Piloting-Battlemech XP before damage when both units are in character, the live target is on another team, and the attacker has a present connected pilot. The award is `max(1, damage / 3)` using reduced glancing damage; misses and trips do not award direct-damage XP. `BattlePhysicalReport.experience` exposes the applied award. The shared catalog policy supplies thresholds and continuous-award behavior. Damage or evacuation failure rolls back XP with the attack. Charge/DFA awards and XP-channel messaging remain unfinished.

`resolve_battle_charge_action` resolves one-way charge damage, recoil and balance with character-aware continuation packets and atomic casualty publication. Accepted packets continue after earlier destruction; eligible target packets award piloting XP before damage, while recoil and later hits on a wreck do not. `BattleChargeReport.experience` exposes these awards. Ordinary charge simulation, mutual charges and movement dispatch remain tactical pending their integration.

`resolve_battle_mutual_charge_action` now publishes both pre-rolled charge outcomes under one character casualty checkpoint. Eligibility and attack rolls are frozen before either collision; a second accepted attack still resolves after first-collision crew loss. Direct awards use live pilot eligibility at each packet, and aggregate notices plus private injuries are published once. Movement dispatch still uses tactical charge entry points pending integration.

Ground and airborne host movement now dispatch one-way and mutual charges through the character-capable resolvers. The movement report retains completed charge reports for private injury publication, then publishes new casualties once. Expected eligibility rejection still clears intent; resolution/publication failures roll back the movement phase. Pure simulation APIs retain tactical admission. DFA and character jump launch remain unfinished.

DFA has a character-capable host entry point, `resolve_battle_dfa_action`, sharing the tactical resolver's packet, recoil and landing rules. Its report retains character injury and flooding consequences for private feedback and atomic crew evacuation. The caller owns jump scheduling and target selection; automatic DFA landing dispatch now uses the same character-capable resolver and publisher. Character pilots can launch ordinary jumps and DFA attacks through the shared native/Lua launch functions. Launch schedules movement; the airborne host action owns subsequent character damage and evacuation.

Character stand commands use `begin_battle_stand_action` through the shared configured adapter. The action publishes fall injuries and newly lethal crew evacuation before committing the attempted rise and its recovery timer. Fatal attempts leave no timer. World-only `begin_battle_stand` remains suitable for tactical simulation without character casualty publication.

The server uses `advance_battle_stagger_action` for timed damage checks. Character units participate in the configured history mode, and failed checks publish character fall injuries and evacuation before the tick commits. Failed publication restores stagger history along with damage, dice and crew locations.

`explode_battle_ammunition_action` detonates an available bin using character-aware cascade resolution and atomically publishes injuries and crew evacuation. Trigger probability remains the caller's responsibility. The thermal scheduler's character integration is still pending.

`advance_battle_overheat_action` resolves due heat injuries, ammunition hazards and shutdown consequences with character health and atomic evacuation. The report retains nested stacking impacts and falls for private feedback. The server thermal phase uses this action within the enclosing tick transaction. Earlier cockpit and unjam feedback is staged first, and all output remains buffered until persistence succeeds.

`resolve_battle_salvo_action` applies a successful grouped weapon hit with character injuries and atomic casualty publication. It stops further groups when the target is destroyed, including pilot death. The firing caller still owns authorization, hit probability, ammunition expenditure, heat and recovery; character shot and command integration remains pending.

`resolve_battle_shot_action` resolves a complete direct shot with character target damage, shooter misloads/recoil and atomic casualty publication. It owns shot expenditure and consequence rollback. Native/Lua firing adapters use the configured character-capable action, retaining their existing observer and aim feedback. The host publishes messages, private injuries and casualties once within the enclosing transaction.

`BattleGunneryExperienceInput::classic_chance` computes the default biped shooting-XP difficulty without consuming RNG or mutating character state. `BattleGunneryExperienceChance::award` applies a supplied 1..50 roll. Character firing uses these calculations when `battletech.xp.oldxpsystem` is nonzero. The shared game configuration selects the alternate battle-value formula, also implemented for supported bipeds.

`BattleUnitExperience` persists per-unit XP scaling and target suppression. `set_battle_unit_experience` is a trusted administrative operation; it rejects negative or nonfinite multipliers. `battle_gunnery_experience_eligible` checks pre-impact biped eligibility for the selected formula without changing dice or balances. Character shots use these settings and checks for classic awards.

`award_battle_classic_gunnery_experience` atomically applies an eligible default-formula award using attacker-owned persisted dice. Its request supplies attack facts and timestamp; identity, tonnage, effective speeds, stored multiplier and skill balances come from the world. Errors restore both RNG and XP. Character firing invokes this operation after each location roll and before the corresponding damage group.

`BattleUnit::effective_maximum_speed` derives the classic XP speed term from current construction mass, template speed, sampled TSM heat and special-map gravity. It preserves integer weight penalties and single-precision speed rounding, and remains distinct from damage-only mobility. Supported constructed bipeds have no cargo, towing, sprint, MASC or supercharger state yet.

`BattleSalvoReport::experience` retains one optional formula-tagged award attempt per applied group. Misses, heat-only fire, coolant and shooter misloads do not invoke target damage awards. Shot, native-command and Lua checkpoints include XP and its RNG consumption alongside ammunition, damage and casualty transfer. Shooting XP diagnostics use the existing channel service.

`BattleWeapon::battle_value` exposes intrinsic catalogue values for all 124 supported weapons. `BattleUnit::battle_value` computes offensive, defensive and total conventional biped BV from installed equipment and current protection, heat capacity and effective movement. It is read-only and requires no pilot. Damage retains installed weapon/bin contributions while changing live protection and movement. The battle-value gunnery formula consumes this score, pilot adjustments and its configuration multipliers immediately before each group.

`award_battle_gunnery_experience` selects the configured formula. Battle-value awards use nominal speed MP, weapon BV/recycle, optional pilot/BTH adjustments, weapon-family and per-unit scaling, and the configured cap. They consume no award dice and honor target suppression. Zero multipliers retain the one-XP minimum. Poor-skill extrapolation can make the square-root arithmetic undefined; Rust records an absent difficulty and explicitly awards the minimum instead of allowing undefined floating-to-integer conversion to affect the shot. Invalid divisors, caps or negative configuration multipliers reject the transaction. Direct, native and Lua character firing all share this path.

`BattleSalvoReport::experience_messages` captures shooting diagnostics before each damage group. Host firing publishes accepted gains to `MechAttackXP`; battle-value mode also emits configured `MechXP` gain/trivial-hit diagnostics. Missing channels are ignored. Existing channel receive policy and history apply, and delivery errors roll back channels, XP, damage and pending output. Classic mode retains its distinct wording and does not emit the optional battle-value diagnostics. Other BattleTech diagnostic call sites remain to be integrated.

Physical attack, charge and DFA experience use the same biped skill selection as piloting checks: `Piloting-Biped` with extended piloting enabled, otherwise `Piloting-Battlemech`. The attack rules carry that selection through each pre-damage award. Native/Lua commands derive it from configuration; direct host operations use their explicit rules.

Physical, charge and DFA reports also retain `experience_messages` for accepted awards. Host consequence publication sends them to `MechPilotXP`, including movement-triggered attacks, under the same checkpoint as damage and casualty transfer. Each message captures the pilot name, amount and selected skill before impact. Missing channels are ignored; delivery failures roll back the whole action.

`BattleUnit::movement_experience()` exposes persisted crossing count and the last piloting XP attempt coordinates. Character ground/jump host updates count successful hex changes, offering one XP every ten updates. A repeated award coordinate is suppressed even across maps; the initial mark is `(0, 0)`. Disconnected pilots accumulate crossings without changing the mark. Awards use the configured biped piloting skill and publish `MechPilotXP` transactionally. Counter overflow rejects the tick instead of wrapping. This counts movement updates that change hex, not every intermediate hex traced within one update.

Physical attack, charge and DFA balance/protection checks now retain optional `BattlePilotingCheck::experience`. Successful actual rolls against targets above two offer XP bounded by the situational modifier; automatic successes and failed/trivial checks do not. This award is independent of damage XP and does not update the movement counter or coordinate mark. The enclosing report publishes accepted gains through `MechPilotXP` under the same action checkpoint. The low-level `roll_piloting` function remains a no-XP primitive; callers explicitly select the award policy. Other control-check call sites remain to be integrated.

Character fall protection checks now award successful-check XP before damage, retaining the mutation in `BattleFallReport::avoidance.experience` and accepted diagnostics in `experience_messages`. The recursive host consequence publisher handles direct and nested falls without duplicating cockpit notices. Failed delivery restores the enclosing action, including prior attacks, dice, XP, damage, channel history and pending output. A failed balance check can therefore be followed by a separate successful protection award; automatic prone protection still consumes no dice and earns no XP.

Damage-triggered balance checks now retain accepted piloting XP on `BattleBalanceReport::check` and diagnostic snapshots in `experience_messages`. The shared impact/salvo/fall consequence traversal publishes them transactionally. Forced falls have no balance roll or balance award; their separate protection roll retains its own XP policy.

Stagger and thermal-shutdown reports now include `experience_messages` for accepted control-check XP. Stagger awards share the history-consumption transaction; shutdown balance awards occur before reactor power-down. Host publication restores history/clocks, dice, power, XP and pending output if delivery fails. Failed balance checks may independently earn protection XP in their resulting falls.

Crowding avoidance now earns configured piloting XP for successful nontrivial checks. Accepted messages travel with collision effects through direct actions, movement, landings and thermal falls. Avoided targets take no damage; channel failures roll back the mover's motion, dice and XP together. Collision mode that applies damage does not also award avoidance XP.

Automatic damaged-gear landings now apply successful-check piloting XP. Landing-local diagnostics merge into the airborne movement report, so a failed channel delivery restores flight, dice, skill XP and stabilization together. The landing result keeps those messages separate from nested fall and collision reports to avoid duplicate publication. Manual early landing remains a separate unfinished character-mode integration path.

Manual `land` and `btech.unit.land` now support character units through `land_battle_jump_action`. The early-abort check can award XP, and successful completion retains subsequent damaged-gear checks and nested combat/terrain consequences. XP messages, injuries and crew evacuation commit with flight completion; delivery or evacuation failure restores the whole action. The pure-world `land_battle_jump` entry point is tactical-only.

Water-entry and reverse-slope control checks now award piloting XP through ground segment results and the movement host checkpoint. Successful reverse-slope entry into water makes one control check, retaining the rule that skips the separate water check. Failed diagnostic delivery restores the position, motion, dice, skill award and pending output together. Landing stagger state remains distinct from ordinary rolling damage history.

### Automatic anti-missile systems

The Rust catalogue supports `IS.Anti-MissileSystem` and `CL.Anti-MissileSystem`,
including half-ton mounts, their normal ammunition, ten-second recycle, one
heat per activation, ammunition explosions and defensive Battle Value.
`ams` toggles the whole unit's saved switch; Lua `btech.unit.ams(unit, pilot,
enabled?)` shares the same control and notification transaction. New units
start with defense off, and direct firing of defensive mounts is rejected.

A launched missile attack selects the first functional, non-recycling AMS mount
on a running enabled target, then the first usable compatible bin, preferring
the mount section. An empty selected mount does not fall through to another
mount. Interception consumes the attacker's persisted dice: one d6 for IS, two
for Clan. Capacity is capped at the incoming rack size; remaining ammunition
caps expenditure rather than interception capacity. Misses still spend supply,
heat and recycle but do not remove any damage. Failed Streak locks do not
activate defense. Successful attacks subtract interception from cluster hits
before regrouping damage, so a defended LRM salvo retains five-point packets.

`BattleShotReport::ams` records the activation and actual intercepted hits;
`BattleSalvoReport::missiles_before_defense` retains the pre-defense cluster
count. Both native and Lua firing publish cockpit feedback within their owning
shot transaction. Callback abort and persistence replay cover defense expenditure
alongside the attack. Critical destruction disables the unit's AMS capability;
flooded equipment is skipped without destroying that capability.

Narc pods, swarm/mining missile modes and their interactions remain outside
the supported missile-defense behavior. Reference behavior was checked
read-only in `combat/mech_combat_missile.c`, `combat/mech_weapon_modes.c`,
`combat/crit_weapons.c`, and the weapon/Battle Value catalogues.


### Laser AMS and defense selection

`IS.LaserAMS` and `CL.LaserAMS` use the same automatic-defense path and controls.
The reference catalogue gives each a half-ton, one-slot mount, 24 rounds per
matching bin and 25-second recycle. IS laser AMS generates 12 heat; Clan laser
AMS generates 1. Both contribute 105 defensive BV; their bins contribute no
positive BV but retain the ordinary ammunition exposure penalty.

Despite the energy classification, the reference defense path requires a
matching ammunition bin for laser AMS too. Rust preserves this observed rule;
without a bin, it does not activate or spend heat. Tests cover every AMS identity
with no bin, empty/short supply, misses, shutdown, critical loss and flooding.
Mixed-mount tests verify ordered selection: a recycling or flooded first mount
can be skipped, but an empty first ready type prevents fallback to another type.
Critical destruction removes capability for the whole defense system.

The Lua weapon union now covers all 128 catalogue identities, with a catalogue
regression guarding both game and fixture annotations against omissions.

Laser AMS follows beam-weapon eligibility for targeting-computer equipment
sizing; conventional AMS is excluded as missile-family equipment. This does
not permit manual fire from either defense type.

## Narc beacons

Conventional IS and Clan Narc launchers resolve non-damaging pod hits inside the
shared shot transaction. `BattleNarcReport` records hit/interception, attachment
section and cockpit notices. Unit state owns the set of marked sections, validates
that each survives, and removes marks during section destruction. Explosive rounds
use normal salvo damage. Narc-compatible missiles use the existing cluster bonus
calculation against marked targets. No bridge or C runtime is involved.

Native `narc`/`explosive` controls and the matching Lua unit methods select ammunition;
weapon expenditure continues to enforce matching bins. ECM suppression and iNarc
variants require their own equipment implementation and remain pending.

## Guardian and Angel ECM

Installed `Ecm` and `AngelEcm` systems now supply the shared electronic-field
calculation. Suite modes and last committed observations live in unit state;
availability is derived from critical slots, damage and power. Native and Lua
controls share a transaction. The server commits field transitions with its
one-second simulation update, and shots query current fields before resolving
Narc/Artemis bonuses and Angel disruption of Streak homing.

Use `ecm`, `eccm`, `angelecm` or `angeleccm`; selecting the active mode disables
that suite. Status and Lua unit inspection expose selected modes and field effects.
Personal suites, stealth/iNarc emitters and advanced sensor/C3 consumers remain
outside the currently supported electronics.

## iNarc

The IS iNarc launcher uses the shared pod resolver and typed per-section beacon
sets. `inarc` and `btech.unit.inarc` select matching homing, explosive, haywire,
ECM or Nemesis ammunition. Attached effects feed weapon aim, compatible missile
clusters and the electronic-field calculation; explosive rounds use salvo damage.
Nemesis selection follows the reference's homing attachment behavior. Pod receipts
include their effect kind, and unit inspection exposes all attached `beacons`.
Manual pod-removal actions remain pending.


## Pod inspection and removal

The Rust pod module owns inspection, arm selection and swatting. Native `pods`
and `removepod` and Lua `btech.unit.pods(unit, pilot)` and
`btech.unit.removepod(unit, pilot, location, type)` use the same authorization and
rules. Read results contain section, destroyed status and attached kinds; removal
results contain the chosen arm, target number, roll, outcome, self-damage and any
nested impact. The host action publishes nested consequences and rolls back the
world and queued effects together if publication or validation fails.

The reference behavior is `show_narc_pods` and `remove_inarc_pods_mech` in
`sensors/mech_electronics_controls.c`. Biped swatting uses piloting +4 and actuator
penalties, does not grant raw-roll XP, and sets 60 seconds of arm recovery after
either outcome. It does not add punch-only posture, shoulder or movement rules.
Conventional Narc cannot be swatted off. Vehicle crew removal is deferred.

## Stealth armor

`BattleSignatureState` owns active status and an optional destination/countdown. Equipment
capability is derived from two passive armor slots in each non-head/non-center
section and installed Guardian ECM. Runtime switching requires working Guardian
parts. The thirty-second event belongs to the normal saved heartbeat, so a failed
world commit restores both the switch and field observation. A switch request is
shared by native `stealth` and Lua; Lua unit inspection returns the saved state.

Active armor contributes ten heat per turn to continuous heat production and
self-interference to the existing electronic field calculation. Weapon previews
and resolved shots share the increased medium, long and extreme range penalties.
Ordinary Guardian emission modes remain independent of armor selection. Power loss
and Guardian damage clear active armor, while an already scheduled destination
remains until its expiry checks power and equipment.

References: `sensors/mech_electronics_controls.c`, `sensors/mech_ecm.c`,
`movement/mech_update_heat.c`, `combat/mech_bth.c`, `combat/crit_mechs.c`, and
`unit/template_specials.c`. Passive armor slots have no separate equipment mass.
The reference's defensive-BV check reads the primary technology bitset rather than
the derived secondary armor flag; this change does not add a new stealth BV bonus.
Probe/C3 interactions await those unsupported systems.

Firing at an active stealth target requires a settled lock on that unit. Missing,
settling or different-target locks reject the shot before ammunition, heat,
recovery or attack dice change. Normal visibility and firing checks still apply.

## Null signature system

Concealment systems share `BattleSignatureState` and its destination/countdown
transition. Stealth armor still depends on Guardian ECM; NSS derives its capability
from `NullSig_Device` in all seven non-head sections and requires every installed
device to remain available. The generic state advances a pending event only at
expiry and consumes unavailable events. Each equipment family owns its validation,
activation requirements and notices.

NSS adds ten heat independently of stealth armor. Either active system changes
range accuracy once; NSS does not add ECM interference or a stable-lock firing
requirement. Damage reconciliation, the server heartbeat, native `nss`, Lua
`btech.unit.nss`, and unit inspection all use this owned state. There is no FFI.

Behavior references are `sensors/mech_electronics_controls.c`,
`unit/template_specials.c`, `unit/template_internals.c`, `combat/crit_mechs.c`,
`combat/mech_bth.c`, and `movement/mech_update_heat.c`. Probe/infrared integration
remains with the pending advanced-sensor work.

## Infrared sensors

Infrared is an optical mode with a fifteen-hex hardware range. It reuses acquisition,
contact retention, primary/secondary selection, delayed switching and map sensor
availability. Its terrain rules permit smoke and water but reject fire, blocked
terrain and six intervening woods. `BattleSensorReport::aim_modifier` and
`BattleSensorAim::modifier` are signed so hot targets can confer an aiming bonus.

World queries derive thermal contrast from current production plus stored weapon
heat and cooling. The terrain-only evaluator documents its cold-target baseline;
`battle_infrared_heat_modifier` evaluates explicit heat rates. Active stealth/NSS
heat contributes normally; neither inherently hides a biped from infrared.
Selection uses native `sensor I I` or Lua `btech.unit.sensors(unit,pilot,"infrared",
"infrared")`. Map controls use the infrared bit independently of visual modes.

References: `sensors/mech_sensor.c`, `sensors/mech_sensor_functions.c` and
`movement/mech_update_heat.c`. Advanced sensor families remain unfinished.

## Seismic sensing foundation

Sensor data types are named for the shared pipeline rather than only its optical
consumers. The first non-optical rule component is `BattleSeismicRules`, whose
inputs explicitly include the scanner's committed signal strength and an attack's
random 0/1 adjustment. `battle_seismic_contact` derives current biped facts without
consuming dice. This separates inspection from the future action that samples and
commits randomness.

Signal strength determines a four-to-eight-hex range in discrete bands. Running,
jump state and the configured stopped-target rule govern eligibility. Aim uses
current physical mass, speed, partial cover and the supplied random adjustment.
The generic report can use the existing contact-acquisition weighting.

References: `sensors/mech_sensor.c`, `sensors/mech_sensor_functions.c`,
`unit/mech_specification_state.c`, and `movement/mech_update_heartbeat.c`.
Seismic cockpit selection remains unavailable until signal updates and attack
randomness are integrated into the saved runtime transactions.

## Runtime seismic integration

`BattleSensorSignal` owns scanner strength and a private independent random stream.
The heartbeat advances running live units and persists the result with other unit
state. Signal updates share the existing rollback boundary, and do not perturb the
combat stream. Configuration supplies the stopped-target policy at host creation
and live reconfiguration; direct scenario code uses `configure_battle_sensor_policy`.

Seismic now participates in native/Lua selection, acquisition, visibility checks
and weapon aim. Aim calculation accepts a candidate dice stream internally. Public
previews clone the unit's stream; firing commits the same candidate only when the
attack succeeds as a domain action. The sensor jitter precedes the attack roll,
and duplicate sensor selections do not consume an extra sample. This structure
also supports future sensor families with stochastic aiming contributions.

The reference signal update is `movement/mech_update_heartbeat.c`; range and
random aiming behavior are in `sensors/mech_sensor.c` and
`sensors/mech_sensor_functions.c`. Electromagnetic sensing and radar are described below; active probes are described below.

## Electromagnetic sensing

EM shares scanner signal state and transaction-owned aiming dice with seismic.
`BattleElectromagneticRules` evaluates explicit target facts, signal and jitter;
`battle_electromagnetic_contact` supplies current terrain, nominal tonnage, movement,
weapon emission and electronic interference. It participates in the ordinary
sensor selector and contact/aim pipeline without a separate cache.

The new `fired_recently` unit marker is set only for launched shots and is persisted
with the unit. The heartbeat clears markers after taking its world checkpoint,
so failed saves restore them. `clear_battle_recent_fire` also supports deterministic
scenario simulation. Shooting previews remain read-only, and the committed shot
consumes the sampled EM modifier before its attack roll.

References: `sensors/mech_sensor.c`, `sensors/mech_sensor_functions.c`,
`combat/mech_fire_resolution.c` and `movement/mech_update_heartbeat.c`. Nominal
weight is deliberately distinct from seismic's current physical mass.


## Radar sensing

`R`/`radar` requires the `AntiAircraft` chassis flag. `BattleRadarTarget` evaluates
explicit integer altitude, surface clearance and flying chassis identity; `battle_radar_contact` supplies
current terrain, flight or fall altitude, equipment and map restrictions. Radar
uses the shared selector, contact cache, acquisition rolls and signed aiming path.
There is no additional sensor state or random aiming adjustment.

Targets must be above altitude two and more than one level above the sensor
surface. Below altitude ten, range must be strictly less than altitude squared.
At altitude ten and above the hardware limit is 180 hexes, but the automatic scan
map ceiling remains until either endpoint reaches altitude eleven. Acquisition
falls from 90 to 10 over the longer ranges. Smoke, fire, water and ECM do not
independently reject radar; blocked terrain does. Aim adds total woods and two for
partial cover, with a minus-three bonus for VTOLs or targets at altitude ten and above. Target woods disappear
when the unit is more than two levels above the terrain datum.

Radar surface height follows sensor terrain semantics: water and bridge depth are
negative, while intact ice uses sea level for units above it. This is deliberately
separate from the standing/deck height used by movement. The map disable bit is
32. Active and pending pairs are validated against equipment on restore.

Behavioral references: `sensors/mech_sensor_functions.c`, `sensors/mech_sensor.c`,
`sensors/mech_sensor_selection.c` and `sensors/mech_los.c`. Aircraft-specific radar
bonuses remain part of the future aircraft implementation.


## Active probes

Beagle (`B`), light Beagle (`A`) and Bloodhound (`H`) share `BattleActiveProbe`
for equipment, range and interference policy. They use the ordinary sensor
selection/contact pipeline and transaction-owned aiming stream. Hardware ranges
are six, three and eight hexes. Aiming adds a uniform zero through two, sampled
once for duplicate primary/secondary selections. Terrain, cover, water, smoke and
fire supply no independent probe obstruction or aim penalty.

Observer ECM disturbance and target Angel protection block every family. Stealth
armor and null signature block Beagle and light probes; Bloodhound penetrates
both. Queries recompute current electronic fields. The acquisition factor 101
bypasses hidden-target range and quarter-chance penalties, while retaining arc,
perception and secondary-slot weighting and the ordinary acquisition roll.

Equipment is derived from `BeagleProbe`, `Light_BAP` and `BloodhoundProbe` slots;
Bloodhound needs at least three biped slots. Any lost/flooded component disables
its family. Damaged active slots fall back to visual, and pending requests for a
broken probe are discarded on completion. Saved pending selections remain valid
while their countdown runs. Beagle/light share map-disable bit 64; Bloodhound
uses 256. The separate legacy light-probe bit is not used by contact eligibility.

Mass follows per-component arithmetic: Beagle 768, light 512, Bloodhound 682 in
1/1024-ton units. Two IS Beagle slots or one Clan slot add ten defensive BV before
movement scaling; light/Bloodhound BV remains absent, matching the reference.

References: `sensors/mech_sensor.c`, `sensors/mech_sensor_functions.c`,
`unit/template_specials.c`, `unit/mech_consistency.c`, `unit/battle_value.c` and
`combat/crit_mechs.c`. Infantry-specific hidden-probe exclusions await infantry.

Target-hex forest cover is resolved once in `unit_terrain_los`, using the target's
integer flight/fall altitude. At two levels above the forest floor the woods still
contribute; above that they do not. Every sensor consuming the terrain report
therefore uses the same canopy boundary. Intervening woods retain their separate
trace count. The behavioral reference is `sensor_woods_count` in
`sensors/mech_sensor_functions.c`.

## TAG targeting

`tag #unit` / `btech.unit.tag(unit, pilot, target)` starts TAG illumination of an
acquired enemy within fifteen hexes and unblocked terrain sight. TAG requires a running
unit, a conscious assigned pilot and working `TAG` components. It does not roll
combat dice. A thirty-second timer prevents changes until the stable-lock notice;
`tag -` or a nil Lua target then releases the connection and starts thirty seconds
of recycling. Equipment occupies one ton per installed slot.

`BattleTagState` belongs to the tagging unit and persists with its other runtime
state. `battle_tagged_by` derives current target ownership and rechecks equipment,
power, objects, map/range and terrain. A new owner displaces the former owner into
recycle; saved duplicate ownership is rejected. No second saved target-side link
can become inconsistent. Retargeting replaces the previous outgoing link.

Explicit shutdown releases TAG in the shutdown transaction. A destroyed device
does not announce readiness when its recycle timer ends. The heartbeat advances
timers and drops invalid connections within its ordinary
world/persistence transaction. Consumers reject invalid connections immediately,
even before heartbeat cleanup. Unit inspection and Lua expose the selection and
timer, with status lines for settling, stable and recycling states. Native and Lua
controls use ordinary world/effect rollback boundaries.

References: `sensors/mech_tag.c`, `unit/mech_lifecycle.c`, `unit/mech_consistency.c`
and `combat/mech_bth.c`. Semi-guided ammunition and its TAG movement-modifier consumer are described below.

## Semi-guided ammunition

`sguided <weapon>` and `btech.unit.sguided(unit, pilot, weapon)` toggle supported
indirect-fire missile launchers between normal and semi-guided rounds. Supported
profiles include IS/Clan LRMs, ELRMs, long-range DFMs and Thunderbolts. Rockets,
other weapon families and one-shot mode changes are rejected. Templates use
`Sguided`; runtime and Lua use `semi_guided`. Supplies retain ordinary capacity,
remain separate from normal rounds and appear as `G` in compact ammunition status.

A current TAG from a different unit on the shooter's team removes positive target
movement penalties and preserves negative modifiers. The TAG need not have reached
its stable-lock notice. Own TAG, enemy TAG, lost contact and broken links provide
no assistance. Aim previews are read-only; firing uses the same current link query
and commits ammunition, dice and damage through the existing transaction.
Semi-guided ammunition does not add a missile cluster bonus.

References: `combat/mech_weapon_modes.c`, `combat/mech_bth.c`,
`unit/weapons_catalogue.c` and `ui/mech_notify_weapon_text.c`. This connects the TAG
lifecycle to supported direct and indirect shots; missing missile families remain
separate work.

## Direct spotter coordination

`spot #own-unit` declares a conventional BattleMech a spotter once its weapon and
limb recycling has completed. `spot #friendly-unit` selects a self-declared
observer from current acquired contacts; `spot -` clears either role. Lua uses
`btech.unit.spot(unit, pilot, observer)` with the own ID or nil for those actions.
The selected ID persists with the unit and appears in status and Lua inspection.

`battle_spotter_target` validates the observer's current role, team, map, power,
consciousness and acquired selected unit target without requiring the firing unit
to see that target. Losing only the firer's contact with an already-selected
observer does not drop the direct link. Invalid links remain inspectable, but the
query rejects them. Selection and queries consume no dice.

Self-declared spotters cannot fire. A unit using another observer must clear that
selection before firing non-indirect weapons. These guards run before shot dice
or expenditure and also cover the direct weapon-expenditure API. Indirect weapon
capability is shared by hotload/semi-guided eligibility, while keeping rockets
excluded from those mode changes.

Reference: `sensors/mech_spot.c` and `combat/mech_fire_command.c`. This provides
coordination and admission for indirect unit shots. Observer-target routing and
spotter aim and spotting/artillery experience are supported.
Delayed artillery data links, radio-range checks and hex spotting depend on the
corresponding unsupported artillery/targeting systems.

### Firing through an observer

After selecting a friendly spotter, clear your own target with `lock -` and use
`fire <weapon number>`. An indirect-capable launcher uses the observer's current
unit target. A personal target lock keeps ordinary direct firing. Indirect aim
reports include `indirect` observer skill, movement and settling contributions;
`optical` then describes the observer's sensors. Ammunition, range and attack dice
belong to the firing unit. Eligible character crews gain one spotting XP for the
observer and one artillery XP for the firer, even on misses, subject to ordinary
skill award timing. Awards and channel diagnostics roll back with the shot.
Artillery/hex spotting remains pending.

Indirect cockpit messages identify the destination coordinates. Observer messages
report fire toward a visible hex without revealing the selected unit or whether
it was hit. The target still receives a known shooter or an incoming-fire bearing.
`battle_hex_visible(world, observer, coordinate)` queries terrain visibility on the
observer's map without acquiring occupants or consuming dice.

### Coordinate locks

Use `lock x y` to select an occupant-search coordinate, or add H (hex), B
(building), I (ignite), or C (clear). `btech.unit.lock_hex(unit, pilot, x, y, mode)`
accepts the same letters or `hex`, `building`, `ignite`, `clear`; omitted mode means
`unit_at_hex`. Valid coordinates can be selected outside visibility and settle in
eight seconds. Selection replaces a unit lock; `lock -` clears either kind.
Lua `target_lock` is either the unit record or a hex/mode/countdown record.
A plain coordinate lock now fires at its current unit occupant using ordinary
firing rules. An empty coordinate uses the hex-shot resolver; H/I/C modes resolve
woodland effects without damaging occupants. Explicit unit targets override
coordinate selections. An established observer still supplies its unit target for indirect
fire when the firer has no personal unit lock.

A coordinate-directed shot report includes `coordinate` alongside the actual
`target` unit. Unit-at-hex selection uses persisted map-slot order and retains friendly
occupants for the firing-safety checks. The selected coordinates stay fixed when
an occupant moves. Indirect aim uses the observer’s lock contribution without
adding a second unstable-lock penalty from the firer.

Battlefield membership order is saved as each placed unit's `map_slot`. New arrivals
reuse the first vacant slot; moving within the map preserves it. The order remains
stable across restart and determines which occupant is selected when several
units share a hex. `battle_map_unit_order(world, map)` exposes that order.

### Inspecting terrain aim

`battle_hex_aim_modifiers` accepts a numeric gunnery target; the pilot variant and
`btech.unit.aim_hex(unit, weapon, x, y)` use current crew skill. The selected
coordinate mode supplies the hex bonus. Reports include `visible` independently
of `subtotal`, which is absent beyond weapon range. They neither acquire units
nor consume dice. Firing separately checks control, readiness, visibility and arcs.
B-mode shots resolve building integrity through entrance references.

### Weapon launch and impact ownership

`weapon_launch` resolves target-independent launch and loader consequences in the
attack's private candidate world. Target defenses and damage follow in the shot
resolver, with recoil afterward. The same launch and recoil operations can serve
terrain attacks without duplicating ammunition, heat, jam or dice rules. Caller
transactions still own publication and rollback.

Woodland attacks separate chance resolution from map mutation. A successful clearing
result can be applied through `apply_woodland_clearing` with the expected source tile;
this preserves elevation and rejects stale results. Occupied maps persist these
reductions through the same terrain dictionary as surface breaks. The surrounding
attack transaction remains responsible for its messages and other consequences.

Smoke markers count down on committed simulation seconds, including on maps with no
active units. Expiration quietly reveals the base terrain. A failed save restores
the countdown along with the rest of the tick; elapsed wall time while stopped does
not consume a saved lifetime. Fire remains a separate spread and burnout lifecycle.


### Native and Lua terrain firing

`fire <weapon number>` and `btech.unit.fire(unit, pilot, weapon)` dispatch selected
H/I/C locks and empty occupant-search coordinates through the shared hex-shot
resolver. Occupied occupant-search coordinates retain ordinary unit admission and
damage. Explicit unit targets, indirect observer selection and self-cooling retain
their existing routing precedence. B-mode resolves building integrity; indirect hex
spotting and mine effects remain pending.

Configured firing returns `BattleFireReport`: either the unit report or a
`BattleHexShotReport` with `map` and `coordinate` and no target unit. Lua receives
the corresponding detached table. Both paths share configured aiming rules,
launch-failure messages and the enclosing world/output checkpoint. Terrain shots
publish coordinate-based observer feedback and woodland notices. Character-enabled
shooters use character-aware misload and recoil consequences, including fall
injuries, piloting experience and casualty evacuation. The direct Rust hex-shot
API remains the non-character simulation entry point.

Tests compare native and Lua ignition, clearing, empty-coordinate firing and Heavy
Gauss recoil, including character crews, aborted callbacks, detached reports,
unchanged occupants and saved state replay.


### Weapon impacts on ice and bridges

Successful H-mode packets now check the base surface after woodland effects. Ice
breaks when a 1–15 shooter roll is at most nominal weapon damage. Bridges use
1–10×(1+elevation); the bridge-capacity map flag suppresses the check. Packet size,
ammunition damage modifiers and glancing do not alter this nominal threshold.
Misses and I/C modes do not make structural checks. Once fractured, later packets
see water and consume no additional structural dice.

`BattleHexShotReport.surfaces` retains each roll and optional fracture report.
Observers who can see the coordinate receive break/shudder notices captured before
terrain changes. Surface occupants fall through the existing surface-break system;
configured native/Lua shots publish character consequences and evacuations inside
the firing checkpoint. Direct tactical calls retain character guards. Building
attacks, indirect hex spotting and mines remain pending.


### Interior-map building state

`StoredBattleMap.building` owns current/maximum construction integrity, building
policy flags and the regeneration factor. The persisted `cf`, `cf_max`,
`build_flag` and `regen_factor` columns now load and save through this checked Rust
state. `set_building_state` updates an interior without replacing terrain or moving
occupants; the caller owns administrative authorization. Asset reloads preserve
construction state. Integrity is bounded by its nonnegative signed-short maximum;
flags retain one byte and regeneration retains its signed integer range.

Policy queries distinguish command centers, complex interiors, hidden/dropship
structures, invisibility and safety. Safety is not treated as damage immunity.
Tests cover occupied updates, invalid values, save/reload, zero integrity,
transactional database failure, terrain reload and corrupt saved integrity.
Entrance references, building weapon damage and repair scheduling are described
below; configuration alone does not schedule repairs.


### Building entrance references

`BattleBuildingEntrance` links a battlefield coordinate to the interior map that
owns construction integrity. `set_building_entrance` configures or removes a stable
ordinal; `StoredBattleMap::building_at` selects the first matching ordinal and
`building_entrances` exposes the ordered entries. Duplicate coordinates and multiple
entrances to one interior are supported. Selection does not filter hidden, safe,
or command-center policies; those belong to the consuming action.

Type-four rows in `btech_map_objects` persist these references. Updates own only
coordinates and the interior reference, preserving auxiliary entrance data and
other object kinds. Invalid coordinates and missing interior maps are rejected.
Terrain reloads retain entrance references, and map purges remove incoming links.
Tests cover ordering, lookup, invalid updates, restart, auxiliary-data preservation,
unrelated object preservation, failed deletion rollback and corrupt references.
Building firing effects and regeneration use these references. Movement through
entrances remains separate integration work.


### Building firing and repair

Native and Lua fire now resolve B-mode successful packets against the first building
entrance. Damage is capped at the interior's remaining integrity; absent and already
ruined buildings are quiet. Complex source maps and command-center targets emit the
paint-scratch response. Packets report `buildings` with interior identity, actual
damage, remaining integrity and captured notices. Shooter, interior contents and
visible destruction observers receive feedback within the firing checkpoint.

The first surviving hit from full integrity schedules repair in 120 committed
seconds. Further damage does not reset that clock. Each event adds the configured
factor, caps integrity at its maximum and reschedules while damaged. Signed factors
use bounded nonnegative integrity arithmetic. Countdown and integrity save together,
including on otherwise idle maps; save failures restore both. Restart resumes the
saved countdown without offline catch-up. Destruction does not schedule rebuilding:
the reference's zero-integrity guard prevents its rebuild branch from being reached.
An already pending repair event stops when it encounters a destroyed building.

Tests cover partial/lethal damage, immunity, absent buildings, misses, continuing
repair clocks, restart replay, full repair, native/Lua character rollback and interior
messages. An idle-server test forces a failed repair save and checks successful retry.
Entrance movement and explicit repair/rebuild administration remain pending.


### Typed minefield definitions

`BattleMinefield` stores coordinate, kind, signed strength, extra setting and owner.
`BattleMineKind` distinguishes standard, inferno, command, vibra and scripted trigger
fields. Stable ordinals preserve ordering and permit multiple fields at one hex.
`set_minefield` performs checked administrative configuration without changing
terrain, occupants or dice; `StoredBattleMap::minefields` exposes the saved records.

Type-three map-object rows persist all definition fields. Updates preserve extension
columns and unrelated map objects. Invalid kinds, bounds and owners are rejected;
map/owner purges remove the corresponding definitions. Asset reloads retain fields.
Tests cover all kinds, ordering, invalid updates, restart, extension preservation,
failed deletion rollback and corrupt kinds.

Weapon clearing deliberately leaves mines intact: the reference's
`mine_field_possibly_remove` performs no removal. A successful woodland-clearing
test verifies this with all five mine kinds. This closes that clearing-parity gap;
Physical activation and trigger callbacks are described below; radio command
detonation remains pending.


### Mine coverage and activation selection

`StoredBattleMap::mine_coverage` derives the coverage union from saved definitions,
without a second mutable bitmap. Standard/inferno/command fields cover their own
hex. Vibra fields use the extra-setting coverage diamond, while trigger fields use
the rounded continuous radius within its coordinate bounds. Negative and zero
settings retain their distinct coverage behavior.

`mine_activations(world, unit, reason)` selects ordered pre-event definitions using
current whole-ton mass and base-terrain altitude. Ice uses surface level zero even
under smoke. Command mines at the unit coordinate return `Spotted`; ordinary fields
return `Explode`. Vibra fields distinguish colocated weight eligibility from remote
weight/range checks. Trigger fields return `Trigger` only for step/land reasons and
sufficient weight. Coverage gates the map-wide scan, so one field can admit a
colocated field that has no coverage of its own.

These are read-only queries: no dice, damage, deletion, notices or callbacks occur.
Tests cover geometry, thresholds, reason exclusions, stable order, smoke/ice,
above-surface rejection, invalid queries and restart equality. The selector is ready
for the physical activation transaction described below; radio detonation remains pending.


### Conventional mine blasts

`resolve_mine_blast` resolves an admitted standard, command or vibra field inside a
candidate world. Each affected biped receives kick-table damage in five-point
packets, with shared immediate critical, balance and flooding consequences. Map-slot
order selects occupants; center damage precedes coordinate-ordered adjacent hexes.
Command/vibra neighbors receive half damage and can ignite undecorated woodland
using the map's saved fire stream. Blast altitude is limited to ground zero and the
level immediately above it; water and ice use signed bottom depth.

Standard fields below strength five are removed. Stronger standard fields retain
their strength; the reference's integer depletion multiplier never updates them.
Command and vibra blasts remove fields at the detonated coordinate, including other
mine definitions there. Reports retain hits, ignition, removed ordinals and notices.
`resolve_mine_blast_action` adds character injuries, casualty evacuation and output
rollback under the host checkpoint. Trigger admission/feedback belongs to the caller.

Tests cover damage packets, neighboring targets/fire, removal, saved replay, late
post-damage failure, mixed character occupants and water-height boundaries. Activation
queries now correctly use signed water depth too. Negative-strength blast behavior,
radio detonation and remaining mine administration remain pending. Inferno
blasts use the burn lifecycle described below; scripted blasts reject before mutation.


### Inferno burns and mine damage

`apply_inferno_burn` extends a bounded, saved duration without an immediate heat
increase. While burning, six points are subtracted from dissipation after water
cooling, floored at zero before environmental temperature adjustments. Committed
simulation ticks age the duration after sampling heat, including in shut-down units.
Expiry publishes the cockpit cooling message once. Failed saves restore the timer
and feedback; restart resumes the saved duration without offline catch-up.

Inferno mines damage occupants at their own coordinate for one third of strength,
in five-point kick-table packets, then apply six burn seconds per strength point.
Strengths one and two still burn despite having no armor damage. Weak fields below
five are removed; stronger fields persist. Damage, burn extension, field removal
and character consequences share the blast checkpoint. Duration overflow rejects
the entire blast, including any earlier occupant damage.

Water entry, landing and falling extinguish burns on water terrain, except for a
standing unit in depth-one water. Extinction cancels the clock, publishes steam
feedback and replaces the hex decoration with 120 seconds of smoke. Burning units
illuminate their own and adjacent hexes. Status heat/info views and Lua inspection
expose remaining burn seconds.

Tests cover cooling, extension, duration bounds, illumination, mine damage/removal,
late failure rollback, water depth/posture, steam, saved replay and a shut-down
server's persistence-failure retry. Negative-strength mine effects and radio detonation remain unfinished.


### Inferno missile exposure

`resolve_inferno_hit` applies an admitted, positive missile count after cluster
selection and interception. Each pair of surviving missiles, rounded up, adds
180 seconds of burning. Repeat exposure extends the clock with the brighter-fire
feedback; first exposure publishes ignition feedback. This operation neither
consumes dice nor changes armor, health or stored heat.

Ignition is followed immediately by the shared water-extinguishing check, so a
submerged target receives ignition and steam feedback while retaining no burn.
Observer messages use acquired, currently visible contacts. A candidate world
contains duration changes and steam decoration together; overflow, invalid targets
and late validation failure leave the caller's world untouched. The detached
`BattleInfernoHit` report retains target, missile count, added duration, extinction
and ordered notices for publication by the enclosing action.

Tests cover paired-missile rounding through the supported count limit, repeated
hits, unchanged unrelated state/dice, observer feedback, water depth/posture, ice,
restart replay, invalid targets, overflow and late rollback after steam creation.
Firing and ammunition-explosion callers share this operation, as described below. Reference behavior: `combat/mech_combat_misc.c` and
`combat/mech_combat_missile.c`.


### Inferno ammunition controls, firing and explosions

Native `inferno <selection>` and Lua `btech.unit.inferno` toggle the shared
ammunition selection, with ordinary pilot/readiness guards and one-shot rejection.
Template `Inferno` flags select typed bins and mounts; capacities, half-ton mass,
matching-bin expenditure and persistence use the existing ammunition model.
Weapon listings show `[Inferno]`, and compact exports use `I`. As in the reference's
actual mode predicate, missile compatibility includes long-range launchers;
Narc/iNarc pod behavior retains its existing priority over burn exposure.

Unit firing computes clusters and AMS interception before applying the shared
inferno hit. Inferno salvos have no armor packets or damage XP; the optional
`salvo.inferno` report contains exposure and feedback. Misses and complete
interception do not burn. Streak lock failure retains its ordinary launch behavior.
Late burn failure rolls back ammunition, recovery, heat, dice and defense effects.
Native and Lua character actions publish the same saved state and notices.

A successful inferno terrain shot invokes one zero-damage terrain exposure rather
than one per missile. It can ignite woods at five or better, including small SRMs,
but cannot clear woods or damage buildings. Ordinary H-mode ice/bridge checks still
occur once. Unit-at-hex targeting retains its existing occupied/empty routing.

Inferno bin explosions apply exposure from one quarter of their nominal potential,
then halve internal damage. With `battletech.inferno_penalty`, they also add thirty
stored heat and use earlier thermal explosion checks: targets 4/6/8/10/12 at heat
10/14/19/23/28. Available inferno bins take priority over larger ordinary bins;
empty or unavailable bins do not activate the penalty. Explosion injury and
character consequences remain in the existing damage transaction.

Tests cover template capacities and mass, control/fire parity and callback rollback,
restart, partial/complete interception, duration overflow, single terrain exposure,
bin damage and water extinction, configurable heat and all thermal thresholds.
Indirect hex fire, other vehicle classes and additional missile families remain
part of their unfinished feature gates.


### Physical mine activation and scripted triggers

Ground hex entry, successful jump landing and completed falls now activate saved
minefields through `activate_mines` and its character-capable transaction. Flat
hex crossings participate even when there is no height, water or ice check. Steps
emit coordinate-specific trigger feedback; falls and landings use the shorter
trigger message. Local command fields publish bomblet spotting without detonation.
Remote vibra detonations announce their visible hex and apply neighboring damage.

Activation snapshots retain ordinal order. Fields deleted by earlier blasts are
skipped. Blast-induced support falls retain their own activation reports, including
further damage to occupants. Movement stops at a triggering step when the unit
falls, shuts down or is destroyed. All movement, dice, damage, burns, field deletion
and queued feedback remain inside the enclosing world checkpoint.

Each selected scripted field queues `on_mech_mine_trigger` on the triggering unit's
attached Lua object module. Its event context uses that unit for object, enactor
and cause, with operation `mine_trigger` and no descriptor. Movement and landing
host actions publish these callbacks together with injury consequences. Callback
failure restores movement, damage, persistent script state and output. Detached
low-level movement functions return notices; the host actions own Lua publication.

`BattleMineEventReport` retains the triggering unit/reason, blasts, callback count
and notices. Fall reports retain their mine event; movement reports retain entry
and landing events. Existing casualty handling visits nested mine injuries and
falls without duplicating their public notices. No new persistence format or C
bridge is required.

Tests cover flat entry, command spotting, inferno burns, restart replay, landing,
fall activation, nested support loss, deletion of later selected fields, remote
vibra feedback, late blast failure and character callback rollback. Radio command
detonation, negative-strength blasts and remaining map administration are still
unfinished.


### Frequency-matched command mine detonation

`detonate_command_mines` resolves the mine phase of an admitted transmission.
It snapshots matching command fields on the sender's map in ordinal order, without
owner, distance or altitude restrictions. The supplied frequency is matched exactly
against each field's signed extra value. Transmission controls own channel limits,
radio readiness and message admission. This operation does not send radio messages.

Each actual detonation captures visible hex feedback, resolves the existing area
blast, then removes colocated definitions. Fields removed by earlier detonations or
nested falls are skipped. Non-command fields and other frequencies/maps remain
untouched except for ordinary colocated deletion and blast consequences. Repeating
a consumed frequency is a no-op with no dice draws or notices.

`BattleCommandMineReport` retains sender, map, frequency, ordered blasts and notices.
`detonate_command_mines_action` publishes character injuries, nested fall effects
and casualties within one host checkpoint. A late blast or publication failure
restores every earlier change and pending notification. Physical activation,
individual blasts and command detonation share the same injury-publication helper.

Tests cover multiple matching fields, deleted later definitions, distant fields,
different owners, map isolation, restart replay, zero/signed frequency values,
invalid senders, no-op repetition, character publication and late failure rollback.
The shared transmission action below encloses this detonation phase with radio delivery. References:
`combat/mine.c` and `ui/mech_notify_radio_config.c`.


### Radio channel configuration

Constructed units save sixteen channel slots, exposing the active subset derived
from template radio quality. Qualities 1–5 provide 2/4/5/8/11 channels and ranges
64/80/100/120/140. Missing or zero quality defaults to 3 for IS and 5 for Clan.
Clan radios and quality 4–5 radios support relay. Unsupported quality is rejected.

`setchannelfreq A=123`, `setchanneltitle A=Command`, `setchannelmode A=DuG` and
`listfreqs` use the shared Rust channel model. Configuration requires a conscious
assigned cockpit pilot and works while shut down. Frequencies are 0–999999;
titles retain up to fifteen bytes without splitting UTF-8 characters. D selects
digital, U mutes, E enables relay with digital/capable hardware. Colors use
`xrgybmcwXRGYBMCW`; the last color wins. Unknown suffixes end mode parsing.
Empty mode resets to analog. Info and scanning modes are unavailable on these radios.

Lua exposes transactional `btech.unit.radio_frequency`, `radio_title` and
`radio_mode`, each taking unit, pilot, zero-based channel and value. `btech.unit.state(unit)`
exposes active settings in `radio` (a one-based array) and derived limits in
`radio_capabilities`. All sixteen saved slots are validated on load. Tests
cover native/Lua state parity, hardware limits, restart, rejected operations and
callback rollback. Changing configuration sends no messages and triggers no mines;
radio delivery and transmission admission use the separate shared action below.


### Digital radio delivery and relays

`resolve_digital_radio(world, sender, channel, message)` computes digital
receptions without mutation, dice draws or publication. It returns the frequency,
map and ordered `BattleRadioReception` records, including the receiving channel,
transmitter path, bearing and formatted cockpit text. `notices()` projects the
result into ordinary cockpit notifications for the enclosing host transaction.

Each unit listens on its first matching unmuted channel, regardless of that
channel's analog/digital selection or the receiver's power state. Destroyed and
pending-removal units do not participate. The transmitter's three-dimensional
range bounds each directed hop. Friendly running units with capable hardware and
a matching relay-enabled channel can forward; muting that channel does not disable
relay. Each unit appears once in the graph, even with multiple matching channels.
Breadth-first search finds the fewest hops without recursive fallback or arbitrary
search cutoffs. Equal-length paths use saved map order deterministically.

Endpoint ECM suppresses remote digital reception, while self-monitoring remains
available. Intermediate relay ECM does not suppress forwarding. Bearings point
toward the last transmitter, colors belong to the receiving channel and titles
belong to the sending channel. Direct paths take priority over relayed paths.

Tests cover multiple hops, equal-length alternatives, directional range boundaries,
shutdown and hostile relays, muted forwarding, first eligible receiving channel,
ECM at each role, formatting and exact restart replay without world mutation.
The shared transmission action below exposes this delivery through `sendchannel`.
Autopilot delivery requires its remaining runtime implementation.
Behavioral reference: `ui/mech_notify_radio.c`.


### Analog radio reception and interference

`resolve_analog_radio` delivers to each same-map unit's first matching unmuted
channel, including frequency zero and powered-down receivers. Receiving channel
mode does not restrict analog reception. There is no hard range cutoff or relay
path for analog: distance beyond sender range causes one interference pass;
distance beyond the larger of receiver range and the average of both ranges
causes a second, weaker pass. Equality at either boundary causes no pass.
Endpoint ECM adds a final pass at distances of at least one hex, excluding self.

The payload includes the sender title before scrambling. Each pass makes
conditional signal and 2d6 communication checks, bounded character displacement,
and printable clamping. ASCII/Latin-1 retain these rules, including space clamping;
larger Unicode characters are retained intact without draws. Receiver color and
channel/bearing framing are applied after interference. Each receiver owns its
saved dice stream, so identical saved state replays exactly. The entire operation
uses a candidate world: a late failure discards all receivers' earlier draws.

Units save `radio_skill`, initially 6, then sample `Comm-Conventional` with the
pilot's mental attributes at startup completion. A player's missing character
profile yields 18; no player pilot yields 6. Later skill changes take effect at
the next startup. Lua's `btech.unit.state` exposes this captured target.

`BattleAnalogRadioReport` includes ordered receptions and `interfered_receivers`
for the enclosing host's XP policy; it does not award XP or publish messages.
Tests cover boundary arithmetic, character/dice order, two-pass range loss,
endpoint ECM, self-monitoring, muting, zero frequency, restart, late rejection
rollback and startup skill sampling. The shared host action applies the communication XP gate described below.


### Native and Lua radio transmission

`sendchannel A=Message` and `btech.unit.radio_send(unit, pilot, 0, message)` use
`send_radio_action`. Both letter cases select the same zero-based channel. The
pilot must be conscious and assigned to that cockpit; crew stun and destroyed
hardware prevent sending. Shutdown does not. Empty text, control characters and
invalid channel indices are rejected before reception or mine effects.

The selected channel chooses analog or digital delivery. Receptions are published
first, then every matching command mine on the sender's map is resolved through
the character-capable mine host action. No receiver or relay is needed for the
mine phase, and frequency zero remains an ordinary exact match. Mine damage uses
the configured hit, stagger, stacking and piloting rules.

One outer checkpoint covers reception dice, all notifications, mine damage and
nested character consequences. A late error restores the pre-transmission world
and outbox. A later error in the enclosing Lua callback also rolls back the whole
action. The returned `BattleRadioTransmission` retains tagged delivery diagnostics
and the command-mine report. Tests verify both modes, native/Lua parity, shutdown
sending, restart, rejected admission and late mine/callback rollback.

Observer scanner privileges and autopilot radio commands remain separate integration work.


### Radio administrative diagnostics

Native and Lua frequency setters use `set_radio_frequency_action`, combining the
setting and `MechFreqs` publication in one checkpoint. A positive frequency emits
one alert for every matching channel on a different-team unit on the same map,
in stable unit/channel order. Muting, shutdown and repeating the same setting do
not suppress alerts. Frequency zero, same-team units and unplaced senders produce
none. This is diagnostic feedback; it does not prohibit sharing a frequency.

A zero-frequency transmission on an in-character map captures a `ZeroFrequencies`
record with player name/id, unit, channel, map and message. The map's flag controls
eligibility independently of the unit's flag. The shared transmission action
publishes that record before cockpit delivery and mine effects, under the same
outer checkpoint. Its returned report includes `audit_messages`.

Both diagnostics use existing ordinary channels and their history, membership and
persistence behavior. Missing channels are ignored rather than created. A channel
publication failure, a later mine failure or an enclosing Lua callback error
restores settings, reception dice, channel counters/history and queued messages.
Tests cover native/Lua parity, duplicate enemy channels, muted/shutdown reception
settings, repeated/zero frequencies, channel overflow, map flags and rollback.


### Communication experience cadence

The transmission host awards one `Comm-Conventional` XP for interfered analog
reception by an in-character unit with a connected player pilot. Accepted awards
publish to `MechXP` and appear in `experience_messages`. Digital and clear analog
reception do not attempt communication XP. The existing skill catalog and ordinary
skill award interval remain authoritative.

Each in-character interfered receiver consumes a saved 61-second gate when due,
even if it has no connected pilot or the skill-level interval declines the award.
A gate still at one second after sixty ticks prevents another attempt. The next
tick permits one. Out-of-character reception does not consume the gate. New units
and completed startups have no pending gate; saved gates survive server restarts,
and offline time does not advance simulation countdowns.

`radio_experience_remaining` is stored with unit state, validated in 0–61 and
exposed in Lua inspection. Shutdown gates keep the simulation tick active and
advance through the ordinary world-save boundary. Award, countdown, diagnostics,
reception dice and subsequent mine consequences share the transmission checkpoint.
Tests verify exact boundaries, shutdown/restart, independent skill throttling,
disconnected pilots, publication/callback rollback and server failed-save retry.


### Observer radio reception

`set_battle_observer` is a trusted scenario edit, with a saved `observer` flag;
radio commands do not let pilots grant themselves this role. Lua inspection
exposes the role and `battlefield_id`. The latter derives from the saved map slot
using base 36 (`AA`, `AB`, … `A9`, `BA`, …), retaining restart identity and slot
reuse without a separate random identity allocator. Labels grow beyond two
characters for large maps. These labels do not yet replace dbrefs in target input
or ordinary contact displays, and are not randomized legacy callsigns.

An observer receives on its first unmuted channel, regardless of tuning. Digital
reception bypasses range, relay-path and endpoint ECM rejection. Analog reception
shows the clear payload while retaining its normal interference draws and XP
eligibility. Both formats identify the source affiliation, battlefield label,
frequency and title, using team color rather than receiving-channel color.
A missing or pending-deletion affiliation renders empty. Fully muted and destroyed
observers receive nothing, and each transmission produces at most one observer
reception. Shutdown does not disable reception.

Tests cover remote/ECM reception, untuned and muted channels, team-color selection,
clear analog text with interference, affiliation removal and exact restart replay.
Observer privileges in scanner/map views remain integration work alongside autopilot radio handling.


### Targeted line-of-sight radio

`radio #123=Message` or `radio AB=Message` and Lua
`btech.unit.radio_target(sender, pilot, target, message)` share a targeted radio
host action. Native labels are case-insensitive and refer to saved battlefield
slot labels; explicit dbrefs match the existing contact-command convention.
Mechs, ground vehicles and VTOLs use the same sender admission and identity
projection. Tests exercise all nine sender/recipient chassis combinations,
including cockpit delivery, persistence and callback rollback. This targeted
operation does not require channel-radio hardware or channel configuration;
channel radio is described below.
The conscious assigned sender must be running, not destroyed and not an observer.
The target must be an acquired contact still visible under current sensor rules.
This operation neither scans nor spends dice, and does not touch channel settings,
communication XP, radio audits or command mines.

The sender receives a confirmation identifying the visible target. A running
target receives a separate message: if it cannot see the sender, the name is
`something`; a visible friendly identity uses lowercase battlefield letters.
Shutdown recipients get no cockpit delivery. Sender and recipient identities are
captured independently, without giving either unit a new contact. All notices use
one output checkpoint, also covered by an enclosing Lua callback transaction.

Tests cover asymmetric identification, native/Lua parity, labels/dbrefs, unchanged
world state, restart, callback rollback, unacquired targets, observer exclusion,
shutdown source/target behavior and invalid text. Autopilot reply handling remains
pending with the autopilot runtime.


### Observer contact notifications

Automatic contact events now project their cockpit notice through
`BattleContactEvent::notice`. Observer mode suppresses ordinary acquisition and
loss chatter while preserving the independent weapon-lock-loss warning. The
server uses this projection through the same staged notification path as other
unit messages. Acquisition rolls, contact state, sensor range, visibility and
lock cancellation are unchanged by observer mode.

Tests compare ordinary and observer scan results and complete saved simulation
state, then force contact loss to verify that only the lock warning remains.
Reloading preserves the notification policy. Observer mode does not automatically
acquire contacts or expand acquisition range. Detailed unit scans are described below;
the tactical display is described below.


### Detailed unit scans

`scan [target] [A|I|W|AIW]` and `btech.unit.scan(unit, pilot, target, options)`
share `scan_battle_unit`. Native targets accept battlefield labels or dbrefs;
omitting the target uses the selected unit lock. The assigned conscious pilot
must have a running, intact unit, functioning scanner hardware and an acquired,
currently visible target with clear terrain LOS. This read-only query consumes
no dice and does not change contacts or notify the target.

Computer grades one through five provide scan/tactical radii of 16, 20, 25, 30
and 35 hexes, with twice that radius for long-range hardware. Zero or missing
quality defaults to grade three for Inner Sphere and five for Clan. One sensor
critical halves the radii (rounding down); two disable them. Detailed scans compare
the truncated spatial distance with the scan radius. Detached Lua unit state
exposes these hardware values as `sensor_ranges`; they do not replace acquisition
sensor limits.

Ordinary armor reports expose integer condition bands and destroyed sections;
weapon reports expose readiness without ammunition, modes or exact recovery times.
Administrator-assigned observers receive exact status and bypass the distance
check, while retaining contact, LOS and hardware requirements. Tests cover grade
and damage limits, armor band boundaries, disclosure, native/Lua parity, authority,
range/visibility failures, unchanged state and saved replay. Tactical/long-range displays remain open.


### Scan warnings

Native `scan` and Lua `btech.unit.scan` use `scan_battle_unit_action` to stage a
running target's cockpit warning with the enclosing transaction. The target's own
contact visibility determines whether it sees the scanner's name or `something`;
friendly identified scanners use lowercase battlefield labels. Observer scans
and shutdown targets produce no warning. The raw `scan_battle_unit` query remains
read-only, allowing inspection without publication. Failed scans and aborted Lua
callbacks leave no staged warning or changed simulation state. Tests cover both
identity views, native/Lua output, observer/shutdown silence and rollback.
Autopilot scan replies remain pending with the autopilot runtime.


### Coordinate unit scans

Native `scan x y` and Lua `btech.unit.scan_hex(unit, pilot, x, y, options)`
inspect the first acquired visible occupant at that coordinate, using saved map
membership order. They exclude the scanner itself and skip unacquired occupants.
An empty hex and a hex containing only unacquired units both return
`You see nobody in the hex!`; scanning never acquires or selects a target.
Coordinates require map bounds, scanner distance and terrain sensor visibility;
the selected occupant then passes detailed unit-scan guards and shares its report
and transactional target-warning path. Observer distance exemptions retain
visibility and hardware requirements.

Tests cover stacked occupants, selection after contact loss, empty/unacquired
privacy, native/Lua parity, saved replay, map bounds, authority, range and visibility.



### Structure scans

`scan x y B` and `btech.unit.scan_building(unit, pilot, x, y)` inspect the first
entrance at a coordinate and read its interior map's current construction factor.
The report is delivered to cockpit occupants. Bounds, visibility, pilot/power and
hardware guards share coordinate scan admission. Explicit structure coordinates
retain the hardware distance limit even for observer units.

Invisible, missing and unavailable interiors give the same no-building reply
without dice or XP. Hidden structures require an in-character scanner with a
connected player pilot. They roll two persisted unit dice against startup-captured
perception plus the rounded coordinate/altitude distance. Failed rolls reveal
nothing. Successful rolls reveal integrity and attempt one Perception XP, subject
to the shared skill interval, with accepted awards reported to MechXP.

`scan_battle_building` accepts an explicit timestamp and commits the domain result
atomically; `scan_battle_building_action` also stages cockpit/channel delivery and
restores the entire state on failure. Tests cover native/Lua output, concealment,
authority, observer distance limits, exact dice use, XP intervals, saved replay and
Lua callback rollback.


### Mine recognition and full hex scans

Native `scan x y H` and Lua `btech.unit.scan_terrain(unit, pilot, x, y)` inspect
buildings first, then mines, within one world/output checkpoint. Structure output
reaches the cockpit. Failed mine recognition is private to the pilot; success
reaches cockpit occupants. Reports disclose recognized mine presence only, never
kind, strength, frequency, owner or trigger settings, and never detonate a field.

Mine recognition requires a field authored at the scanned coordinate. Empty hexes
consume no dice. Occupied coordinates draw an inclusive 2..9 range gate, compared
with truncated spatial distance, before testing in-character perception eligibility.
A passing gate permits a two-die check against cached perception with no distance
modifier. Successful recognition attempts one Perception XP through the same
interval and diagnostic path as hidden-building scans. Both phases share one
wall-clock timestamp; an accepted building award can suppress the mine XP award
without suppressing detection.

`scan_battle_mines` accepts an explicit timestamp and commits domain state atomically;
`scan_battle_hex_action` includes both scans and all output. Tests cover dice order,
empty/OOC/far outcomes, recognition, private versus cockpit delivery, native/Lua
parity, unchanged fields, saved replay and callback rollback of both phases.



### Scanning saved targets

A plain `scan`, or a report option such as `scan A`, now uses the complete saved
target selection. Unit locks inspect that unit; building locks inspect the
structure; full-hex locks inspect buildings then mines. Unit-at-hex, ignition and
clearing locks inspect the first acquired visible occupant. A scan works while
the target is still settling and never replaces the lock or advances its countdown.
Missing selections produce `No default target set!`.

`scan_battle_selected_action` and Lua `btech.unit.scan_selected(unit, pilot, options)`
share dispatch and return a tagged unit/building/hex report. Coordinate selections
recheck clear terrain LOS and retain the appropriate scan admission checks.
Observer units may bypass hardware distance for selected coordinates; explicit
`scan x y B/H` keeps its range limit. Observer status does not bypass visibility,
disabled hardware, or perception eligibility. Selected structure reports publish
through their existing transactional actions, including rollback of dice and XP.
Tests cover all five coordinate modes, native/Lua report and output parity,
unchanged countdowns, restart, distance/visibility checks and callback rollback.


### Brief unit reports

`report [target]` and `btech.unit.report(unit, pilot, target)` share the read-only
`report_battle_unit` summary used at the top of detailed scans. Reports show identity,
range/bearing, speed/heading, coordinates, heat and unit type without detailed armor,
weapons or ammunition. They require the assigned conscious pilot, a running unit,
working scanners and an acquired visible target with clear terrain LOS. Direct
reports do not enforce detailed scan distance and never warn the target or spend dice.

Native `report x y` selects the first acquired visible occupant in saved map order,
subject to coordinate range/visibility limits. A plain `report` uses the selected
unit or the occupant of the actual saved hex coordinate without changing the lock.
Tests cover native/Lua parity, disclosure, silence, direct versus coordinate range,
empty/unacquired occupants, saved selections, authority, visibility and restart.

The reference scan/report handlers accept map coordinates, not a separate projected
bearing/distance scan form. Earlier projected-scan notes have been removed; navigation
and tactical-map projection use the shared display-center resolver described below.


### Display center resolution

`resolve_battle_view_center` and `parse_battle_view_center` share tactical and
long-range display centering. Requests select the own unit, an acquired visible
contact, or an integer compass bearing and signed distance from current continuous
position. Negative distances reverse the compass. Ordinary projection limits use
the absolute truncated distance; observer projections bypass that limit. Contact
centering always retains spatial range and current contact visibility checks,
even for observers. All modes require a conscious assigned pilot, running unit
and working display hardware.

Lua `btech.unit.view_center(unit, pilot, kind, arguments)` returns the map, center
and damage-adjusted hardware radius. It changes no simulation state, target lock,
contacts or dice. Projected centers may lie outside the map; a renderer must clip
its viewport and apply terrain/contact visibility separately. Extreme or nonfinite
projections fail within the shared geometry bounds. Tests cover compass wrapping,
signed/fractional range boundaries, labels/dbrefs, observer policy, hardware loss,
visibility, authority, detached Lua parity and saved replay.

This supplies display center resolution only. Tactical and long-range rendering,
overlays and player display preferences remain unfinished; viewport sizing is described below.


### Bounded display viewports

`resolve_battle_viewport` and Lua `btech.unit.viewport(unit, pilot, kind, arguments,
dimensions)` resolve an in-bounds rectangle from the shared display center.
Requested tactical dimensions default to 21 by 14 and validate within 5..40 by
5..24. Both axes then cap at twice the damaged tactical radius and the map size.
Long-range height defaults to 11, validates within 10..40, caps to hardware/map
height, and rounds up to an odd height before final map clipping. Its horizontal
span includes 71 cells, matching the reference's inclusive 70-column span.

The returned origin, width and height use count semantics and stay inside even
one-cell maps. Extreme projected centers clamp to the nearest usable edge without
overflow. Requested dimensions are supplied per call; persistent player preferences
remain separate work. Tests cover boundaries, small maps, damage, odd heights,
projection clipping, invalid dimensions, Lua parity, unchanged state and restart.
This is renderer input only; terrain/contact disclosure and text rendering remain
unfinished.


### Long-range terrain, elevation and unit maps

Native `lrsmap T|E|M [target|bearing distance]` and Lua
`btech.unit.lrsmap(unit, pilot, mode, arguments)` now render bounded long-range
maps using shared centering and viewport rules. T shows effective terrain,
including fire/smoke overlays; E shows numeric elevation/depth with blank zeroes;
M overlays currently acquired visible units on terrain. The scanner uses `*`,
friendly bipeds `b` and hostile bipeds `B`. Stacked contacts use saved map order,
with the scanner's own marker taking precedence. Other unit classes remain gated
by construction support.

Rows stagger odd and even columns across two text lines with column and row labels.
Dark maps mask evaluated hexes failing current terrain visibility with `?`, while acquired
visible contact markers retain their independent unit visibility. Ordinary maps
show terrain independently of contact acquisition. Rendering changes no contacts,
locks, dice, timers or output. Tests cover native/Lua parity, exact row positions,
water depth/zero elevation, map edges, overlays, friendly/enemy markers, missing
contacts, dark-map masking, observer projections, authority and saved replay.

The finer seen/terrain/elevation LOS masks remain unfinished. Dark-map rendering currently uses the shared boolean hex
visibility query. Tactical rendering and saved display preferences also remain
integration work.


### Long-range map styles

The shared map palette now supplies terrain, own-unit and relative-contact styles.
ANSI-enabled pilots receive colored T/M displays: water depth distinguishes bright
from ordinary blue, woods distinguish bright from ordinary green, fire is bright
red, own-unit markers are bold, friends bright yellow and enemies bright red.
Plain T/M output keeps identical visible glyphs and spacing. E remains uncolored
and uses blank zero elevation; C (`colored_elevation` in Lua) applies terrain colors
to numeric elevation and displays zero explicitly. Client capability handling still
owns final ANSI/HTML output.

Cell data is escaped independently from trusted palette markup. Adjacent styles
are coalesced, and both staggered rows reset before coordinate labels or the next
line. Evaluated but obscured dark-map cells use blue question marks. Tests compare styled/plain display
geometry, own/friendly markers, fire and zero elevation, label reset boundaries,
ANSI preferences, native/Lua parity and saved replay. Configurable palettes are
not introduced; the reference's default map palette is shared for future renderers.


### Explicit long-range visibility modes

`lrsmap L`, `H` and `S` apply the current terrain visibility query on any map:
L displays visible terrain, H visible elevation, and S visible units over filtered
terrain. Lua accepts those letters or `visible_terrain`, `visible_elevation` and
`visible_units`. H retains uncolored elevation and blank zeroes; L/S honor the
pilot's ANSI preference for visible cells. Acquired unit markers remain governed
by their independent contact visibility, with the own-unit marker available in S.

Every requested cell is evaluated. An obscured result therefore uses the reference's
blue `?` marker, replacing the earlier red `X` used for these cells. X denotes an
unevaluated cell in the reference; this renderer evaluates its complete viewport.
Tests cover ordinary-map filtering, all three native/Lua modes, contact nondisclosure,
own markers, unchanged state, restart and restoration of visibility.

The shared boolean terrain query still does not implement the reference's separate
terrain/elevation mask trace, directional terrain sensor ranges or full illuminated
terrain tracing. Those remain required work for complete display LOS parity.


### Live terrain illumination

`battle_hex_illuminated(world, map, coordinate)` now derives lighting from active
fire decorations, inferno-burning units and forward searchlights. Fire and inferno
sources light their own hex and six neighbors. Searchlight terrain beams use the
current heading/torso arc, strict sixty-hex spatial reach, and the shared terrain
obstruction query. Destroyed or unavailable lamps do not contribute. Smoke,
blocked terrain and excessive woods prevent beam illumination. The terrain beam
range is separate from the existing thirty-hex unit-illumination query.

Hex visibility now passes this live lighting into optical eligibility, so visual
night range and light-amplification glare follow the same rules as illuminated
unit detection. This applies to visibility-filtered long-range maps and other
callers of the shared hex query. Queries are read-only, do not identify lighting
sources, and never acquire contacts or consume dice. Tests cover fire neighborhoods,
source removal, inferno expiry, forward/backward beams, distant lit terrain,
obstructions, unavailable units, display output and saved replay.

Directional terrain sensor ranges and the full terrain/elevation mask tracer
remain unfinished. Illumination currently uses the owned shared terrain geometry
rather than the reference's separate skyline beam trace.


### Tactical hex displays

`battle_tactical_map(world, unit, pilot, arguments, dimensions)` renders a bounded
hex canvas with terrain, elevation, coordinate labels and acquired contact IDs.
Native `tactical [C|T|B|M|L|U] [target | bearing range]` and Lua
`btech.unit.tactical(unit, pilot, arguments?)` use default display dimensions and
return the same styled text. Lua additionally returns the clipped viewport.
Shared display admission enforces the conscious assigned pilot, running unit,
operational tactical hardware and center/range rules. Views keep global column
parity when clipped; dimensions are counts, including small maps.

The standard view uses live fire/smoke decoration; U shows the base terrain in
the lower half. L and dark maps hide unseen terrain with question marks using
the shared terrain-visibility query. Acquired live contacts retain two-character
battlefield labels, lowercase for friends and uppercase for enemies; the source
is `**`. Stacked contacts use stable map order, with the source taking priority.
Labels longer than two characters are clipped to the cell width. Palette colors
follow the pilot's ANSI flag. Text glyphs are escaped separately from styles.

Rendering changes no contacts, targets, dice, simulation state or outbox. Tests
cover native/Lua parity, restart, unseen-contact privacy, authority, projection,
underlying terrain, colors, elevation and clipped odd-column alignment.
Navigation-shaped views, configurable
player display preferences, multihex units and richer terrain/elevation LOS
masks remain unfinished. This is a tactical display milestone, not full btech parity.


Tactical `C` and `T` overlays mark elevation differences of at least three and
two levels respectively. Water and ice use negative depth from base terrain;
smoke/fire decorations do not change cliff geometry. Each shared southern edge
is evaluated once and only between tiles inside the clipped viewport. Elevation
moves to the top half and the lower half becomes a hex edge. Other contacts are
omitted; the own-unit marker remains. Plain displays use `|`, `!` and `,` edge
markers; ANSI displays use red backslashes, slashes and underscores. Dark maps
reject these overlays. Native/Lua and persistence checks cover signed depth,
thresholds, decorations, marker disclosure, color and dark-map rejection.


### Landing suitability and tactical B

`tactical B` and `btech.unit.tactical(unit, pilot, 'B')` show landing suitability
in the lower-left corner of each hex: green O for ready, red X for unsuitable
(plain O/X without ANSI). The own-unit marker remains; other contacts are omitted.
Dark maps reject the overlay. The current strict policy requires base grass or
road terrain and all six neighbors on the map at the same raw elevation.
Neighbor terrain itself is unrestricted; fire/smoke decoration does not change
base suitability. This display does not implement aircraft landing physics.

`StoredBattleMap::landing_suitability(coordinate, team)` returns Ready,
ImproperTerrain, UnevenGround or Blocked in that order. Circular
`BattleLandingExclusion` records carry coordinate, radius, exempt team and owner.
`set_battle_landing_exclusion(world, map, ordinal, zone?)` is the trusted Rust
administrative setter; None removes a record. A nonzero matching team bypasses
that individual circle, while team zero exempts nobody. Overlapping restrictions
are independently enforced and the radius boundary is included. Definitions
are validated, retained on terrain reload, removed with their owner, and persisted
in map-object type 9, without rewriting mine or entrance rows. Radius must be
nonnegative. Native/Lua administrative authoring and the permissive landing-policy
configuration remain integration work.

Tests cover terrain and neighbor rules, decorations, edge rejection, circle radius,
team exemptions, invalid edits, read-only display state, native/Lua parity, save/load,
exclusion removal, and dark-map rejection.


### Tactical mine overlay

`tactical M` and `btech.unit.tactical(unit, pilot, 'M')` move terrain elevation to
the top half of each hex and clear the lower half for a `<>` mine marker. Contact
labels and the own-unit marker are drawn afterward so elevation cannot overwrite
an ID. Only the first authored field at each coordinate participates; a trigger
field in that position suppresses the marker even when a later explosive field
shares the hex. Standard, inferno, command and vibra fields use the same marker.
No strength, trigger parameters, owner or mine kind is disclosed.

Markers require live terrain sensor visibility and unblocked terrain LOS. Dark
maps accept this mode and retain normal terrain masking. The overlay does not
use perception checks, consume dice, award experience, trigger mines, acquire
contacts, change targets or publish notices. Tests cover ordering, trigger
suppression, visibility, dark maps, native/Lua parity, restart, clipped odd-column
elevation placement and unchanged simulation/outbox state. The shared terrain LOS
limitations still apply; navigation-shaped displays remain unfinished.


### Current hex center navigation

Native `findcenter` and Lua `btech.unit.findcenter(unit, pilot)` share
`find_battle_hex_center(world, unit, pilot)`. The report contains the current hex,
unit elevation, horizontal range from continuous motion to the hex center, and
clockwise integer bearing. Native text uses two decimal places for range; Lua
also retains the precise numeric measurement. Coincident positions report 180
degrees by the established readout convention. Extra native arguments do not
change the current-hex measurement.

Admission requires the conscious assigned cockpit pilot and a running, surviving
unit. Scanner hardware and target acquisition are unnecessary. Rendering changes
no motion, contacts, targets, dice or notifications. Regression coverage includes
continuous offset, zero range, disabled scanners, native/Lua parity, restart,
wrong-pilot and shutdown rejection. The combined `navigate` display is described below.


### Combined navigation display

Native `navigate [target | bearing range]`, Lua
`btech.unit.navigate(unit, pilot, arguments?)`, and
`battle_navigate(world, unit, pilot, arguments)` share a thirteen-line display.
The right side is a radius-two hex-shaped local map, reusing tactical terrain,
contact filtering and palette rendering. The view stays centered at map edges,
retaining off-map outline cells rather than shifting or aliasing terrain.

The left compass plots continuous positions inside the requested center hex:
acquired friends are x, enemies X, and the source is drawn last as *. Contact IDs
remain on the local map. Position/elevation, effective terrain, speed and heading
readouts always describe the source, even for a remote display center. Supported
BattleMechs report zero sustained vertical speed; aircraft controls remain open.
The normal conscious-pilot/running-unit checks apply. Failed scanner hardware
still permits the own-hex view; explicit centers retain hardware range limits
and existing observer projection exemptions. Dark-map terrain masking is shared.

Tests cover both column parities, a one-hex map, off-map surroundings and remote
projections, continuous contact placement, enemy/friendly filtering, disabled
hardware, native/Lua parity, restart and unchanged simulation/outbox state.
Fine terrain/elevation LOS masks, multihex units and aircraft-specific navigation
remain unfinished. This does not complete the broader btech integration.


### Saved player map dimensions

`battle_view_dimensions(world, player)` reads saved dimensions or standard defaults;
`set_battle_view_dimensions(world, player, dimensions)` validates and replaces them
for a live player. Trusted host code owns authorization for changing the selected
player. Lua `btech.player.view_dimensions(player, dimensions?)` queries when the
second argument is omitted and replaces when supplied. Missing fields in a
replacement use standard defaults. Callback failures roll back edits.

Native `tactical`/`lrsmap` and their Lua counterparts now use saved player sizes.
Lua `unit.viewport` uses saved sizes for omitted fields, while explicit Rust
renderer dimensions remain caller overrides. `navigate` retains its fixed radius-two
shape. Bounds remain tactical width 5–40, height 5–24 and LRS height 10–40;
hardware and map bounds still clip the request, and LRS keeps its odd-row adjustment.
A default-sized replacement resets dimensions without deleting configuration.

Sizing persists in the existing player configuration record. New rows initialize
required configuration defaults; previously absent contact-filter fields receive
their defaults when UI preferences become active. Existing filter values, loadout
fields and technician schedules are preserved. Invalid active dimensions reject
load, and player deletion purges the owned state. Contact-filter behavior remains separate integration work.

Tests cover defaults, invalid edits and callback rollback, native/Lua map sizing,
viewport defaults, restart, reset, unrelated-column preservation and invalid database
values. Display queries do not mutate preferences or publish notices.


### Self-service display sizing

`mapdisplay` reports the invoking player's saved tactical width/height and LRS
height. `mapdisplay width height lrs-height` validates and replaces all three;
`mapdisplay reset` restores standard dimensions. This native self-service command
is available outside the cockpit and has no target-player argument. It delegates
to the same preference setter used by Lua, rejects switches and invalid dimensions
before mutation, and changes no unit state. It is a Rust-facing convenience over
the reference's Lua configuration interface, not a newly claimed reference command.

Tests cover query without mutation, valid edits, every dimension boundary failure,
malformed arguments, attempts to supply another player, rejected switches, reset,
Lua agreement, and restart persistence while outside the cockpit.


### Saved unit-contact inclusion preferences

`contacts +` applies saved unit-list preferences. Bare `contacts` retains its
unfiltered acquired-contact list, and explicit `contacts #unit` remains a direct
lookup. `BattleContactPreferences` includes dead, shutdown, enemy, ally and selected
target categories. The defaults exclude wrecks and include the other categories.
A selected unit with include_target enabled bypasses the category exclusions,
but live acquisition and sensor visibility are always required. Wreck inclusion
is independent of shutdown inclusion. No contact or target state is changed.

Rust exposes `battle_contact_preferences`, `set_battle_contact_preferences` and
`filtered_battle_contacts`. Lua `btech.player.contact_preferences(player, value?)`
queries or replaces saved settings inside trusted callbacks. Lua
`btech.unit.contacts(unit, preferences?)` accepts an explicit preference table;
omitting it retains the existing unfiltered API. Use the player getter to supply
saved filters. Replacement fields omitted in Lua use category defaults.

Player dimensions and contact preferences share one typed saved record. Each
group is updated independently in the player configuration table; loadout and
technician columns remain separate. Building policy belongs to contact preferences. Tests cover ally/enemy categories,
shutdown/wreck distinctions, selected-target exemptions, visibility, native/Lua
behavior, callback rollback and independent sizing persistence. Native editing of
saved categories remains open; per-call options and building contacts follow below.


### Per-call unit contact options

`contacts options` accepts d (wrecks), s (shutdown), e (enemies), a (allies),
t (selected target), and b (buildings). An explicit option word starts with no categories enabled.
`!` resets all unit categories on and makes all subsequent letters exclusions;
a later `!` resets the categories again. Unknown characters produce ordered
`Ignoring … as contact option.` diagnostics without changing the exclusion mode.
Only the first fifty characters are processed. Parsing does not edit saved settings.
Examples: `as` includes allies and shutdown allies; `!s` includes everything except
shutdown units (wrecks and the selected target retain their independent policies).

`parse_battle_contact_options` and Lua `btech.player.contact_options(options)`
return both preferences and ignored characters. Pass the returned preferences
to `btech.unit.contacts` for matching unit filtering. Native `contacts +` continues
to use saved preferences, and bare contacts remains unfiltered. Building option b
invokes the identification-lock path described below. Tests
cover sticky exclusion, resets, case, length bounds, warnings, native/Lua parity,
selected-target behavior, visibility and unchanged saved state across restart.


### Building contacts and identification locks

`contacts b` lists visible structures; combine b with unit categories, such as
`contacts bas`, for a mixed list. Lua `btech.unit.building_contacts(unit, pilot)`
and Rust `battle_building_contacts(scripts, unit, pilot)` return detached structure
reports. The new `IDENTIFY_BUILDING` / `identify_building` lock identifies the
interior map, with enactor/subject set to the pilot, cause set to the observing
unit, no descriptor, and silent=true. No denial message is automatically emitted.
The normal lock default is to allow when no handler is registered.

Unavailable and invisible structures are excluded before invoking a lock. A denied
hidden structure is omitted; an ordinary denied structure remains visible with x,
or X for a command center. Safe structures use X, and identified command centers C.
Reports include entrance coordinates, elevation, range/bearing, plain name, CF and
maximum CF, identification result and hidden marker. Source admission requires a
conscious cockpit occupant and a running surviving unit. Visibility uses the shared
terrain sensor/LOS path; range is measured to base elevation plus one. The fine
building-height sensor trace remains part of the broader LOS work.

Entrances are evaluated in saved order. Candidate validity and visibility are
checked again after each lock because callbacks may change state. Lock errors
restore the whole operation's world state and pending notices; successful callbacks
retain their deliberate effects. The query itself rolls no dice or perception
checks. At most 250 structure reports are retained; native combined output admits
buildings only while its contact count is below that limit. Existing brief-mode
sorting and sensor/weapon-arc prefixes remain open.
Tests cover native/Lua parity, lock identities, default/deny/concealment behavior,
invisible exclusion, callback rollback, visibility, authority and restart.


### Saved building inclusion

`BattleContactPreferences.buildings` is a `BattleBuildingContactMode`: `Include`,
`Exclude` (the default), or `FollowBrief`. Lua uses `"include"`, `"exclude"`, and
`"follow_brief"` respectively. For example, a trusted callback can use
`btech.player.contact_preferences(player, {buildings="include"})` to include
structures in `contacts +`. The replacement still defaults omitted unit categories.
Explicit `contacts b` is independent of this saved setting. Pure unit-list queries
ignore building policy; use the separate structure query for detached reports.

The shared mode resolver accepts whether the current brief display includes
buildings. Native output resolves the saved unit contact mode: mode 1 includes
buildings, while modes 0, 2 and 3 omit them unless explicitly requested.
Persistence owns the existing buildings column (0 follow, 1 include, 2 exclude),
rejects invalid values, and preserves unrelated player configuration. Tests cover
all modes, native/Lua behavior, restart, sizing independence, invalid values and
callback rollback. Exact contact formatting remains open.


### Unit brief settings

`brief` queries the cockpit unit's display choices. `brief C 0..3` selects the
contact mode; `brief A 0..6` selects the routine-notice mode. Letters are case
insensitive and may touch the number. The two settings are independent and survive
restart with the unit. A conscious cockpit occupant may edit them even with engines
off. Edits notify occupants; queries are read-only. Lua
`btech.unit.brief(unit, pilot, arguments?)` shares admission and atomic notices.

New units use C1/A0. C1 starts bare contacts and explicit option strings with
building inclusion; ! clears it, and b can explicitly add buildings before !.
Saved contacts + resolves its include/exclude/follow_brief setting against C1.
Rust parse_battle_contact_options_for_display and the optional second Lua
player.contact_options argument supply the initial building policy.

A6 suppresses routine contact notices; A2/A3/A5 restrict them to enemies.
These choices never suppress weapon-lock loss warnings or alter acquisition.
The current shared contact text is retained: exact verbose/short/shorter layouts,
sorting, arc/sensor prefixes and automatic-notice color/text variants remain open.
Tests cover default inclusion, all modes, explicit exclusions, independent edits,
invalid input/state, authorization, callback rollback, lock warnings and restart.


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

Contact modes C0–C2 frame lists with `Line of Sight Contacts:` and
`End Contact List`, including empty lists. C3 emits only rows, and emits nothing
for an empty list. Ignored-option diagnostics precede the list without a trailing
blank line when C3 is empty. This behavior is covered across all modes and after
persistence reload. Framing does not change detached Lua contact rows. The C0
verbose row layout and lateral heading remain open.

C0 now renders multiline biped reports with full plain names, battlefield labels,
tonnage, range/bearing, speed/heading, coordinates, excess heat, movement type and
observer-relative arc. Destruction, shutdown, fallen and jumping notices are
independent; jumping includes the committed path heading. Like the reference,
C0 uses generic names through blocking terrain while still reporting these
physical details. Native output escapes the complete plain report and Lua exposes
`verbose_text` alongside `short_text`. Tests cover exact layout, native/Lua parity,
restart, name escaping, combined conditions and jump headings. Lateral heading
and non-biped movement classes remain open.

Biped lateral movement now uses a durable active mode and six-second pending
transition. The assigned pilot needs Maneuvering_Ace. Native `lateral` and guarded
Lua `unit.lateral` accept directional aliases, replace pending requests, and
cancel them when the active mode is requested. The simulation tick advances
changes even at rest; expiration while stopped discards the request. Restart
retains the countdown. Ground projection, domino collision relative speed and
contact/report headings use the lateral travel axis; weapon arcs retain chassis
facing. Rust exports BattleLateralMode, BattleLateralState, set_battle_lateral
and battle_lateral; Lua unit state exposes lateral. Tests cover eligibility,
commands, rollback, alias parsing, replacement/cancellation, timing, restart,
actual movement, contact displays, shutdown expiry and invalid saved countdowns.
Vehicle admission and movement rules remain pending; quad admission is implemented below.

Maneuvering Ace turning controls now support native `turnmode` and guarded Lua
`unit.turnmode`. Tight/normal selects the durable unit preference; empty or other
arguments report it. Mutation and occupant notification roll back together.
The assigned pilot and saved Maneuvering_Ace advantage are required, while the
control does not require running engines. Under slowdown mode two an unfinished
heading change subtracts 0.4 movement points (4.3 KPH) after the angle-based speed
multiplier. Other slowdown modes and straight movement are unaffected. Tests
cover access, query purity, native/Lua edits, callback rollback, restart and
movement comparisons across slowdown modes. Charge damage retains chassis
heading, as in the reference; domino impacts use the lateral travel axis.

Cockpit status info now shows the committed lateral direction when active and
the TIGHT/NORMAL turn mode when the assigned pilot currently has Maneuvering_Ace.
Pending lateral changes remain absent until activation. The displayed chassis
heading remains independent of travel direction. Short/export and non-info
sections are unchanged. Tests cover native/Lua parity, query purity, transition
timing, restart and loss of pilot eligibility without erasing saved preferences.

Biped bootlegger maneuvers now use native `bootlegger` and guarded Lua
`unit.bootlegger`, with a read-only Rust battle_bootlegger_modifier query.
Admission checks assigned pilot, running power, placement, 43 KPH forward speed,
intact legs and leg/weapon recycling. The situational modifier combines speed,
tonnage, submerged water and leg weapon penalties, with hip precedence and a
minimum of one. A shared piloting roll succeeds with a 90-degree heading change,
half current speed, retained desired speed and 30-second leg recycling. Failure
uses shared tactical or character falls at the modifier severity. Notifications,
XP, injuries and casualty transfers use one rollback checkpoint. Tests exercise
success and failure with seeded dice, replay, persistence, native/Lua controls,
character XP, readiness errors and complete callback rollback. Other unit classes
and detailed diagnostic-channel wording remain outside this biped slice.

Movement ETA now supports explicit integer x/y coordinates or the selected
ordinary hex, including a settling hex lock. Building, ignition, clearing and
unit targets are not defaults. Rust battle_eta is read-only; native `eta` and
Lua unit.eta publish the detached report to cockpit occupants atomically.
Horizontal range and absolute current speed determine whole minutes; speeds
below 0.1 KPH report Never. Very large estimates saturate at the reference signed
integer limit. Explicit coordinates can lie outside map bounds, and sensor
hardware is not required. Tests cover forward/reverse/near-zero speeds, hour
formatting, defaults, passengers, native/Lua output, rollback and persistence.

Compass bearing now supports native `bearing` and read-only Lua unit.bearing,
backed by Rust battle_bearing and BattleBearingReport. No arguments measures to
the selected live visible unit or selected hex of any purpose. Two integers use
the current continuous position; four supply origin and destination hex centers.
Coordinates are bounded to the current map, with the reference bearing-only
exception permitting an explicit origin on the east border. Coincident points
report 180 degrees. Unit defaults require retained acquisition and current sensor
eligibility; explicit coordinates and hex defaults do not require sensors.
Tests cover all forms, exact native/Lua text, passengers, bounds, query purity,
restart, loss of unit visibility and continuous sub-hex motion. Range/vector
commands remain open.

Native `range` and Lua unit.range_report now share validated navigation
endpoints with bearing through navigation_measurement. They accept default
targets, destination coordinates, or explicit origin/destination coordinates.
Spatial range uses continuous live unit height and signed water/ice bed heights.
Dark maps hide coordinate height differences while preserving unit-target
height. Ground range is shown only when it differs after one-decimal rounding.
The existing unit.range(first, second) geometry query is unchanged. Unlike
bearing, terrain-backed origins must name a valid tile when heights are read;
dark-map horizontal queries do not read that tile. Tests cover terrain signs,
dark masking, target visibility, native/Lua parity, query purity and restart.
Vector remains open.

Native `vector` and Lua unit.vector now share endpoint admission and range
formatting with bearing/range. Zero, two and four coordinates use default or
terrain-backed endpoints; three or six integers provide explicit x/y/z heights.
Reports include horizontal/spatial distances, compass bearing and a signed
vertical angle rounded away from zero. Text includes the vertical mark only for
default targets and explicit elevations. Vector keeps terrain heights on dark
maps, matching the reference. Tests cover every form, sign and vertical-only
geometry, bounds, explicit east-border origins, native/Lua text, query purity,
restart and target visibility.

Heading and speed now support private no-argument current-value queries.
Lua unit.heading/unit.speed likewise return the actual numeric value when the
third argument is omitted, while edits retain boolean success and cockpit
notification. Native and Lua speed requests share walk/cruise, run/flank, stop
and back names and finite numeric clamping to the current forward/reverse
envelope. Lower-level set_battle_speed remains a strict validated setter.
Queries allow cockpit passengers; changing controls still requires the assigned
pilot and existing movement checks. Tests distinguish actual from desired motion,
exercise alias/limit behavior, reject nonfinite/type errors, and cover rollback
and restart. The reference command catalog reverses the cruise/flank description;
implementation follows the actual speed table (cruise=walk, flank=run).

Cockpit stun prevents new forward speed requests above walking speed (with the
same 0.1 kph running threshold used by unjamming and water restrictions). Walking,
reversing and stopping remain available. Native commands and Lua use the same
control check, including after a restart; running becomes available when the
saved stun countdown expires.

Ammunition dumping is available through `dump` and Lua
`btech.unit.dump(unit, pilot, selection)`. Select `all`, a zero-based weapon number,
a section (`RT`, `Right_Torso`, etc.), or a section and one-based critical slot.
`stop` cancels an active attempt. Selecting `all` can replace a narrower attempt;
other overlapping requests are rejected. Weapon selection includes bins of every
ammunition mode for that weapon family.

Dumping starts without spending ammunition. Each committed second ejects
`floor(rounds_per_ton / 30)` rounds from every selected usable bin when capacity is
at least 30. Lower capacities eject one round every `floor(30 / rounds_per_ton)`
seconds. This integer cadence follows the reference rate, with the phase owned by
the saved attempt rather than the process-global C event clock. Shutdown or unit
loss cancels ejection on the next step; damage and firing can exhaust selected
bins before completion. Running and jumping are blocked while dumping. Cockpit
and visible-observer start/stop/completion messages share the host transaction,
as do low-ammunition warnings. Lua unit state exposes `dumping` for inspection.

Rear-torso weapon hits can ignite one selected, nonempty, functional ammunition
bin. The ignited salvo strikes rear armor through the ordinary damage cascade,
then one round is removed and dumping stops. The reference executable applies
this without a probability gate, despite a contradictory comment. Physical,
fall and nested explosion damage do not initiate another dump ignition. Reports
expose `dump_ignitions`; saved dice replay bin selection and subsequent damage.
Unavailable or empty bins cannot supply the ignited round.

MASC hardware is represented by `Masc` critical slots and exposed through
`masc_installed` and `masc_operational` in Lua unit state. Installation requires
`max(1, tons / 20)` slots for Inner Sphere units or `max(1, tons / 25)` for Clan
units, using integer division. An explicit `Masc` special without sufficient
hardware does not grant capability. Undersized and surplus installations retain
their physical slots; operational capability counts surviving, unflooded slots.
Each installed slot weighs one ton while its section remains present, including
a damaged slot. Booster runtime controls are described below.

`masc` and `btech.unit.masc(unit, pilot)` toggle an operational powered MASC
installation, scaling desired forward or reverse speed by 4/3 on activation and
3/4 on deactivation. The throttle, effective-speed and movement-update paths
retain their distinct booster calculations. Weapon/status output shows the
current counter and On/Off state; Lua unit state includes the durable `masc`
record.

Activation schedules a check after one committed second, then every 60 seconds.
The required rolls are 3+, 5+, 7+, 9+, 11+, then 13+; early checks retain the
reference's wizard-pilot protection. Turning MASC off reduces its counter once
per 60 seconds. Shutdown clears activation and overload history. Failure
permanently disables the booster, breaks both hips, stops movement, and causes a
one-level fall when absolute current speed exceeds 10.75 kph. Falls reuse the
ordinary tactical/character consequence handling. Timers, rolls, damage and
notifications share the host commit and rollback boundary. Colored counter formatting remains open.

`scharge` and `btech.unit.supercharger(unit, pilot)` control the compressor.
`SuperCharger_Tech` is the authoritative installation flag; `SuperCharger` is its
physical critical part. This follows the reference, including flag-only designs
and ordinary critical loss retaining the technology. The critical-weight switch
assigns no separate mass to the compressor. Overload failure permanently clears
its operational capability, destroys compressor slots in the center torso, and
rolls one to four surviving center-torso engine criticals in slot order. Existing
engine damage rules supply heat and destruction consequences.

MASC and the supercharger share `BattleBoosterState` and the single
`advance_battle_boosters_action` host update. Each keeps independent overload and
recovery history. Either active device penalizes the other's check by one before
wizard protection. Simultaneous checks retain their scheduling order across saves.
One active device raises the effective running ceiling to 4/3; both raise it to
5/3. Successive toggle commands still scale requested throttle by 4/3 each, so the
saved motion envelope accommodates that temporary overshoot. Movement updates
retain the reference's separate combined-booster rounding. Lua state and cockpit
weapon/status output expose both devices independently.

When TSM is hot, its movement-update conversion replaces the additional booster
adjustment. The control ceiling still includes active boosters; turning retains
its separate additive TSM allowance. Saved motion bounds account for TSM rounding
before, between, or after booster toggles, so valid hot forward/reverse throttle
transitions can be saved and replayed through cooling.

C3 computer hardware is represented by typed `C3Master`, `C3Slave`, and `C3i`
critical slots. `BattleUnit::c3_hardware()` and Lua `unit.state(...).c3_hardware`
report installed and working hardware independently of reactor power and network
membership. A biped master uses five slots grouped in slot order within one
section (gaps are allowed); each complete group fails independently. Slaves need
one working slot, while C3i needs two working slots anywhere in the chassis.
Destroyed or flooded slots cannot contribute to working hardware. Technology
flags alone do not supply computers. Physical mass is one ton per master/slave
slot and 1.25 tons per C3i slot, retained after critical damage while the section
survives. Incomplete installations remain representable but cannot operate.
Classic C3 membership and displays are described below.


C3i membership now uses a shared saved network identity rather than replicated
peer lists. Native `c3i <ID>` and Lua `unit.c3i(unit, pilot, ID)` join a visible
friendly running unit's network; `-` disconnects. Networks hold six units total.
A unit must disconnect before joining another network. The assigned conscious
pilot needs a running unit with functional C3i and no hostile ECM disturbance.
Shutdown and interference retain membership; damaged computers, team changes,
and leaving a battlefield remove it. Moving within the same map retains it.
`battle_c3i_members` and Lua `unit.state(...).c3i_members` return the eligible
members including the queried unit, or an empty list for a disconnected unit.
Membership inspection does not imply that a shutdown or jammed peer can supply
targeting data. Join notifications and all membership changes share the existing
host transaction, including Lua callback rollback and database restart. Saved
networks reject more than six members or mixed map/team identities.



C3i now supplies range assistance to conventional unit and hex aim, including
actual shots. Aim reports keep `distance` as physical spatial range and add
`network_range = { distance, source }` when the C3i calculation is active. The
closest running, unjammed member with a visible acquired target supplies unit
range; terrain targets use a clear terrain sightline. A null source means no peer
improves the shooter's range. Equal distances retain the first member in stable
object order. Peer consciousness does not disable this automatic calculation.
The shooter still needs its own firing visibility (or an admitted indirect
spotter); a network sighting alone cannot authorize a shot.

Physical distance still determines minimum-range penalties, including hotload
settings, and the configured outer weapon limit. A usable C3i network disables
extended range using the reference's rounded physical long-range check. Peer
proximity never adds a minimum-range penalty. The shared bracket then receives
the existing target stealth/null-signature modifier. Network coordinate sightings
require positive x/y coordinates, matching the reference's border restriction.
Shutdown or ECM at a peer prevents its assistance without deleting membership;
ECM at the shooter uses ordinary aim instead. These queries do not consume dice,
acquire contacts, or mutate membership. Save/load reproduces both assisted aim
and the resulting shot exactly.


Native `c3imessage <text>` and Lua `unit.c3i_message(unit, pilot, text)` send to
running, unjammed C3i peers with conscious pilots, then echo to the speaker's
cockpit. A physical network must exist even when all peers are temporarily
unavailable; in that case the speaker still receives the echo. These messages
ignore radio tuning, radio range, and acquired contacts. Peers see
`C3i/Name [ID]: text`, and the speaker sees `C3i/You: text`, both in bold. Leading
ASCII whitespace is removed; trailing text is retained. Message text and names
are literal, so bracket sequences cannot change formatting. Preparation is pure,
consumes no dice, and does not change network membership. The native and Lua
adapters stage every recipient under the same transaction and roll back all
output on failure. `prepare_battle_c3i_message` exposes recipient notices for
inspection without publishing them. 


Native `c3inetwork` and Lua `unit.c3i_network(unit, pilot)` now provide the C3i
network status display. The typed report includes unit IDs, lowercase battlefield
labels, names, coordinates, elevation, spatial range, bearing, actual signed
speed, chassis heading, and remaining armor/internal percentages. Armor includes
rear protection; all construction sections contribute to the original totals.
Rows show running, unjammed peers in stable object order, excluding the requester.
Consciousness does not suppress automatic status reporting, and visual contact
is not required. Names are limited to twelve characters in the compact text;
typed rows retain the full plain name. The native report is private to the pilot;
Lua returns the same report without output. Connected networks with no available
peers retain the header and footer. Reports neither change state nor consume dice
and reproduce after restart. 


Native `c3itargets` and Lua `unit.c3i_targets(unit, pilot)` report the union of the
pilot's direct contacts and sightings from running, unjammed C3i peers. A clear
sighting anywhere in that network reveals the name and five condition columns;
otherwise the row shows `something` and blank conditions. P/S markers always
represent the requesting unit's own sensors, and weapon arcs and physical range
are measured from that unit. Each row also includes shared targeting distance
and the assisting peer, using the same selection routine as weapon aim.
Network-only sightings do not become acquired contacts or authorize firing.

The compact report uses eleven-character names, chassis heading, physical `r:`
and network `c:` range columns. Selected targets are red; identified opponents
are yellow. Unidentified labels are lowercase, matching the network display's
convention. Rows sort by descending physical range with destroyed targets first;
equal keys retain battlefield order. Visibility is computed independently for
each row, so unseen entries cannot inherit preceding targets' visibility. Reports
are private in native commands and returned without output in Lua, preserve all
state and random streams, and reproduce after restart. 


Classic C3 membership now shares the command-network engine with C3i. Native
`c3 <ID|->`, Lua `unit.c3(unit, pilot, target)`, `battle_c3_members`, and Lua state
`c3_members` expose it independently of C3i. The reference implements a shared
membership group, not a persisted parent/child hierarchy. Capacity is one plus
three times the number of working master computers, capped at twelve total
units. A second computer on one chassis contributes another three slots. Two
slave-only units cannot start a network, while a master can join through any
visible friendly member. Admission includes the joining master's capacity.

Damage or departure reduces available capacity. The Rust model retains working
masters first, then slaves in stable object order, giving every observer the same
membership set. Excess members become unavailable immediately and their unused
identities are cleared on the next membership change. This canonical ordering
replaces the reference's observer-dependent replicated-list trimming. Queries
remain pure, and saved damage reproduces the same retained set after restart.
Loss of every master installed on a chassis disables its classic C3 even if that
chassis also has an intact slave. Shutdown retains membership; team changes,
map departures, and failed hardware disconnect the affected network. C3 and C3i
use separate saved identities, so leaving one does not leave the other.

C3i continues to use its six-unit limit and
existing targeting, message, and display adapters through the shared engine.


Classic C3 now participates in conventional unit and hex targeting through the
same range calculation as C3i. The aim report's `network_range.kind` identifies
`c3` or `c3i`. Classic C3 takes priority when both memberships are present; a
closer C3i peer cannot override it. Temporarily unavailable classic peers do not
cause a switch to C3i. Leaving classic C3 or losing its hardware permits C3i to
supply assistance again.

The temporary usable view excludes shutdown and jammed computers and recalculates
classic capacity from the remaining working masters. It retains the requester,
then working masters, then peers in stable object order, matching the reference's
requester-relative capacity check. This temporary view never erases durable
membership. Peer consciousness does not suppress automatic targeting assistance.
Physical reach, minimum range, hotload rules, visibility admission, and the ban
on network extended range remain shared across the two families. Both terrain
aim and actual unit shots report the chosen family and assisting peer; saved
shots replay exactly. 


Classic network messaging is available through native `c3message <text>`, Lua
`unit.c3_message(unit, pilot, text)`, and pure `prepare_battle_c3_message`.
C3 and C3i use one delivery implementation with independent audiences and the
appropriate `C3/Name [ID]` / `C3/You` prefixes. The temporary recipient view excludes
shutdown, jammed, and unconscious peers. For classic C3, an unavailable master
also reduces recipient capacity; the requester and remaining masters retain
priority. Unlike automatic targeting, an unconscious master does not support
message delivery capacity. These temporary restrictions do not delete saved
membership. Radio tuning and acquired contacts are not required. Literal text,
sender echo, multi-recipient rollback, and restart parity share the C3i behavior.


### Classic C3 network and target displays

Native `c3network` and `c3targets`, plus Lua `unit.c3_network(unit, pilot)` and
`unit.c3_targets(unit, pilot)`, now use the same status/target report implementation
as C3i. Both families return `BattleNetworkStatusReport` or
`BattleNetworkTargetReport` through their family-specific Rust entry points.
Classic target rows identify `network_range.kind` as `c3`, independently of any
simultaneous C3i membership.

Available peers are selected after shutdown and ECM filtering, then limited by
active master capacity. Unconscious masters still supply automatic status and
target data. These temporary views preserve saved membership. Status needs no
acquired contact; target reports combine only the selected family's usable
sightings and the requester's direct contacts. Displays do not acquire contacts,
change state, or consume dice. Native output is private to the requesting pilot;
Lua returns detached rows and text without sending messages.

### Laser heat-sink designation

`LaserHS_Tech` is accepted and retained in constructed and saved templates. The
reference lists this technology but implements no distinct thermal, mass, or
damage behavior for it. It therefore leaves the existing chassis rules intact:
Clan sinks use two-slot groups and two points of cooling per sink; ordinary IS
sinks remain single unless `DoubleHS` is also present. Environmental cooling and
critical losses use those same rules. No separate laser-sink simulation is added.

Four unchanged Night Gyr variants now pass construction. NightGyr-B still
fails its incomplete two-slot heat-sink installation check. Tests cover both
single and double sink designation, unchanged mass/cooling, grouped critical
losses, and preservation through database save/load.

### Template sensor and radio ranges

Constructed bipeds accept `Tac_Range`, `LRS_Range`, `Scan_Range`, and
`Radio_Range`. A positive value replaces only that field's quality-derived
range. Missing or zero values retain the computer/radio quality defaults,
including Clan defaults. One sensor critical halves each sensor range with
integer truncation; two disable all three. Sensor hits do not reduce radio reach.

Scan admission, tactical/LRS centering, radio delivery, and detached Lua state
all use these derived limits. They remain reproducible after save/load without
storing duplicate runtime range values. Sensor ranges accept 0..127 and radio
range accepts 0..32767; malformed and out-of-domain values are rejected rather
than silently truncated. `RadioType` selects the capabilities described below.

### Explicit radio capabilities, info, and frequency scanning

`RadioType` accepts the packed configuration byte: the low nibble selects 0..15
active channels, and the high nibble enables relay (1), info (2), scanning (4),
or disables digital reception/transmission (8). Missing or zero configuration
uses the quality/chassis defaults. Radio range remains independent. Native and
Lua mode controls enforce these capabilities; info and relay require digital
mode. `I` enables info and `S` scanning. The channel list displays scanning ahead
of color or info, and mode confirmation retains I/U/E/S order.

Info receivers see a `{R-path:...}` prefix on relayed digital messages, containing
each intermediate relay's battlefield ID and bearing toward its predecessor.
Direct traffic has no path prefix. Analog-only receivers and relays cannot carry
digital traffic, even with saved digital mode settings.

When no unmuted channel matches an analog transmission, scan-capable receivers
search on each scanning channel. Each attempt uses the receiver's persisted dice:
detection probability is min(80, message bytes) percent, followed on success by
a 1..min(99, message bytes) percentage step toward the frequency, with a minimum
one-unit step. Search ignores range, ECM, shutdown, and channel muting. Digital
and zero-frequency broadcasts never search. An already exact scanning frequency
needs no search, avoiding the reference's zero-difference division edge case.

Analog reports retain frequency changes and ordered cockpit notifications.
Frequency updates, dice, scanning notices, reception, XP, and command-mine effects
share the transmission checkpoint and roll back together. Scanning channels are
excluded from opposing-team manual-frequency audit matches. Lua exposes both the
new capabilities and saved mode flags.

### Chassis anatomy and quadruped decoding

`BattleMechChassis` separates anatomy from the stable eight section identities.
It supplies chassis-specific section names, leg membership, and critical-slot
capacity. Biped front attachments are arms with twelve slots; quad front
attachments are legs with six. Both use six-slot head/rear-leg sections and
12-slot torsos. The existing section enum retains its biped identifier names;
code interpreting limb roles must use chassis anatomy, and quad labels come from
`section_name` rather than the biped `BattleSection::name`.

The asset decoder now accepts complete quad section layouts and resolves them
after all fields are read, so `Move_Type` may appear after section data. Mixed
biped/quad headings and duplicate sections are rejected. Loadout diagnostic
labels use the parsed chassis. Template checks expose `chassis` to Rust and Lua.

Quad construction now uses the shared BattleMech runtime. The unchanged Scorpion
asset has live coverage for placement, startup, acceleration, standing, weapon
fire, front-leg kicks, and database replay. Equipment validation still rejects
unsupported or malformed installations. This does not imply complete parity
for every optional command or chassis technology.

### Quad mobility and piloting components

Damage-derived mobility now uses chassis leg roles. With one unavailable leg a
quad loses 10.75 maximum speed and gains +2 damage difficulty, cancelling its
separate -2 chassis piloting advantage. Two lost legs leave maximum speed 10.75
and +7 damage difficulty (net +5). Three or four leave zero speed and +2 damage
difficulty. Flooded legs count as unavailable support. Immediate fall and
movement dispatch for those losses remain part of quad runtime integration.

Each surviving hip hit halves speed, including multiple hips on a quad; the
biped two-hip immobilization rule remains biped-specific. Hip damage suppresses
lesser actuator penalties on that leg. Other damaged leg actuators each subtract
10.75 and add +1 difficulty, in stable section order. Gyro disablement and unit
destruction still prevent motion. Shallow-water cooling counts functioning sinks
in all four legs and excludes flooded equipment from both base cooling and the
immersion bonus.

Chassis skill selection is shared by piloting targets and movement, physical,
and successful-check XP. Extended mode selects `Piloting-Quad` or
`Piloting-Biped`; ordinary mode uses `Piloting-Battlemech`. The quad -2 advantage
is included in control-check targets before damage, including default unpiloted
skill. Raw skill inputs for attacks and battle-value calculations remain separate.
Component tests cover all sixteen leg-loss combinations, flooding, hip/actuator
suppression, gyro disablement, and serialized damage. 

### Quad acceleration and lateral travel

Ground acceleration/braking now uses one twentieth of the current maximum for
bipeds and one tenth for quads. The assigned pilot's `Speed_Demon` boolean
advantage increases either rate by 25 percent. This affects convergence toward
the selected speed, not the maximum itself. Live biped tests verify acceleration,
braking, canonical boolean values and saved fractional-motion replay.

Active quad lateral travel applies `(maximum - 10.75) / maximum` to the target
speed after terrain, heat and turning adjustments, preserving reverse travel.
If maximum is at most 10.75, lateral travel stops. An intact quad can request
lateral movement without Maneuvering Ace; that advantage permits requests even
with unavailable support. Requests retain the shared six-second timer.

Destroying a quad leg clears the active lateral offset. A pending request for
another direction keeps its countdown; a now-redundant pending request for no
offset is cancelled. Component tests verify front-leg admission/loss and saved
lateral state. 

Standing and landing now share chassis-aware support counts. An intact quad
stands without a control roll; one or two unavailable legs require a roll,
and three or four prevent standing. A destroyed gyro still prevents standing.
Automatic standing consumes no dice or XP and retains crew/power restrictions
and the existing stand target and recovery rules. Three unavailable quad legs
prevent an upright landing; repeated hip hits alone do not. Biped two-hip
landing failure remains unchanged. Flooding and balance checks classify quad
front attachments as legs and flooding notices use chassis section names.
These rules now participate in live quad construction and actions.

Hit-table selection now requires the target chassis explicitly. Quad punch
and kick tables distinguish front/rear leg pairs and side attacks. Weapon
hit distributions remain shared. Physical strikes, jump attacks, collisions,
mines, partial-cover hits, NARC pods and head grazes all pass the affected
unit's chassis. Head-graze rerolls retain the same dice and stun policy.
This completes chassis selection for these tables; quad attack admission,
geometry and displays still require integration before live construction.

Weapon arc queries now require chassis anatomy. All four quad leg mounts
ignore stored torso offsets, while front-leg mounts retain their respective
side arcs and rear-mount precedence. Both unit-target and hex-target firing
use this shared geometry. Quad torso-rotation commands are rejected.
Quad physical-attack restrictions and remaining runtime integration are open.

Physical attacks now select their limb from the attacking chassis. Quad kicks
and trips use the front pair for actuator penalties, weapon-recycle checks and
limb recovery. They require an available attacking leg, at least three usable
legs and working hips on remaining support. Punches, clubs and hand-weapon
attacks reject quads. Chassis/support tests cover both attack sides, every
flooded-leg combination and hip loss in each leg. 

Charge selection uses all chassis legs: a quad may have one unavailable leg,
while a biped needs both. Flooded legs count as unavailable support. One-way
charge aim compares the attacker's raw skill with the defender's control skill,
including the quad bonus; mutual charges compare raw skills on both sides.
The bonus therefore makes a quad defender two points harder to hit in a one-way
charge without changing mutual-charge difficulty. Tests cover all chassis/role
combinations, both skill modes and serialized component state. 

Quad front-leg mounts now use leg accuracy modifiers: upper/lower/foot damage
adds one each, while hip damage suppresses those modifiers. Both unit and hex
firing reject submerged front-leg mounts until underwater combat is implemented.
Prone quads with no unavailable legs can fire without propping. With one or two
unavailable legs, rear-leg weapons are blocked and other mounts use the shared
front-support checks. Three or four unavailable legs block all prone firing.
This follows the reference's actual cutoff (despite its three-leg message).
Tests cover every damage combination and section.

MASC failure now seizes every chassis hip, including both quad front legs.
The existing saved MASC failure state also forces zero ground speed and prevents
an upright landing. This remains distinct from ordinary four-hip damage on a
quad, which progressively halves speed. Shutdown and serialization preserve
the permanent failure. 

Armor status, weapon status, scan output and compact weapon exports now use
chassis section names. Quad front and rear legs are labeled explicitly, including
limb-recovery and critical/explosion feedback. Critical observer messages also
classify front-leg actuator effects as leg effects. Component status tests verify
labels and read-only snapshots.

Bootlegger turns now check and recycle all chassis legs, including quad fronts.
Their actuator difficulty reuses the shared mounting modifier instead of a
second calculation. Jump landing warnings count front-leg loss, and cockpit
critical messages classify front actuators as leg equipment. Component tests
cover damage, flooding and recovery in each quad leg. Live construction is enabled; the broader parity audit continues.

### Live quad construction

Nineteen quad assets now construct through the normal runtime, bringing the
asset audit to 1,208/1,745 with no previously accepted templates regressing.
Nine decoded quad assets remain rejected for engine installation, unsupported
A-Pods, or heat-sink capacity. Live Scorpion tests cover startup, placement,
acceleration, fall/automatic standing, weapon resolution, front-leg kick recovery,
world validation and database replay. Other unit classes and broader BattleTech
feature parity remain ongoing work.

Live quads reject club acquisition before arm/terrain checks, using the shared
`grabclub` action. Direct, native and Lua tests verify the same rejection and
unchanged BattleTech state. Explicit dropping retains its existing behavior.

Dump and pod-removal commands resolve locations through chassis anatomy.
Quad legs accept `FLL`/`FLLEG`, `FRL`/`FRLEG`, `RLL`/`RLLEG`, and
`RRL`/`RRLEG`, plus full front/rear leg names. Biped selectors retain their
own arm/leg names; mixed anatomy is rejected. Dump slot bounds follow section
capacity. Parser tests and a live front-leg ammunition-bin scenario verify
selection, capacity rejection and unchanged state on failure.

Quads may inspect attached pods but cannot swat off iNARC pods. The shared
removal action rejects this before consuming dice or changing recovery, pods or
protection. Live direct/native/Lua tests verify rejection and unchanged state;
pod status uses chassis-specific front/rear leg labels.

### Asymmetric engine installations

BGS-1T's CT/LT/RT engine counts are 6/5/1; BGS-2T's are 6/4/2.
The reference derives overlapping flags, but its implemented mass and combat
calculations resolve these to XL and XXL respectively: XL wins mass precedence
over XXL, while XXL wins over Light; both Inner Sphere defensive factors are
0.5. Rust now accepts these layouts and their mirrored forms using that effective
family. Their actual engine slots still determine critical damage and destruction.
The owned definition retains the layout for future economy/repair work, whose
flag precedence may differ. GOL-3S remains unsupported because an engine critical
is installed in a leg. Tests verify numeric mass, damage, serialization and the
unchanged Barghest assets.

### Low declared double-sink cooling

Double-sink designs may declare fewer than twenty cooling points. Construction
preserves even capacities from ten upward rather than imposing an extra twenty
point minimum. Installed group completeness and declared-versus-installed
capacity checks remain enforced. Clan technology does not silently double the
asset's declared dissipation. Tests cover capacities 10 through 20 on unchanged
Snow Fox layouts, mass/cooling serialization and invalid capacity rejection.
The asset audit now accepts 1,216/1,745 templates, adding ARC-2RC, Icestorm-1,
Loki-A, SnowFox-1 and SnowFox-2 with no prior acceptance regressions.

### A-Pods

Inner Sphere and Clan A-Pods use the shared Rust weapon catalogue and combat
pipeline: one critical slot, half a ton, range one, no ammunition or heat, zero
armor damage, and a thirty-second recycle. This matches the reference's generic
beam-weapon behavior; there is no separate anti-infantry detonation implementation
in the reference. Each installed pod adds one defensive Battle Value before the
movement multiplier. It also retains the reference's ordinary offensive value of
one, subject to the shared heat-budget calculation. A-Pods are not AMS and do not
intercept missiles. Existing asset spellings, serialization and Lua weapon values
are supported without a C bridge.

### Artillery flight and impact rules

`BattleArtilleryFlight` captures launch coordinates, weapon identity, ammunition
mode and the attack's hit/miss result. Its validated serialized countdown advances
in committed seconds: five hexes per second, truncated, with a ten-second minimum.
Arrival uses the current map bounds and wind strength and returns an ordered
`BattleArtilleryImpactPattern`. Countdown and random draws remain unchanged when
arrival validation fails; an arrived cursor cannot emit another impact.

Standard rounds produce five-point damage packets in the center and half damage
in adjacent cells. Cluster rounds produce damage-count two-point bomblets with
punch locations, using the reference's triangular distribution over a five-by-five
area. Placement conditions that distribution on map bounds without retry loops.
Smoke produces independently rolled 90–150 second effects in the center and its
neighbors; mine ammunition produces one local field-strength effect. Miss scatter
uses the reference integer wind weighting, rectangular coordinate offsets and
edge clamping. An attack that scatters back onto its target remains a miss for
subsequent friendly trajectory adjustment.

The configured firing action connects these rules to ammunition expenditure,
map-owned queues, transactional server ticks and saved trajectory correction.

### Artillery arrival application

`advance_artillery_flight` now composes the cursor with the map's saved random
stream and applies arriving patterns in one candidate world. Standard and cluster
blasts use battlefield membership order and existing tactical damage/critical,
pilot injury, balance and flooding resolution. Standard packets use weapon hit
locations; cluster packets use punch locations. Each cell's blast originates at
its own center. Occupants at or above ten levels over ground zero or at or below
four levels underneath it are excluded; water/ice ground zero is the surface.

Smoke replaces the current overlay while preserving base terrain. Mine rounds
create a neutral standard minefield only when the cell has no existing field.
`advance_artillery_flight_action` additionally publishes notices and character
injuries/evacuation with rollback of world state, effects and the supplied cursor.
The domain-only entry point rejects affected in-character occupants, including
when encountered after earlier packets. Impact failure restores map and unit dice.

World impact effects use existing database persistence. The raw cursor API leaves
ownership to its caller; the queue API below supplies durable ownership and
automatic ticking. Native and Lua firing use the configured launch action below.

### Persistent artillery queues

Each map now owns an ordered queue of admitted rounds. `enqueue_artillery` checks
that launch coordinates match the placed shooter and caps each map at 4,096 rounds.
It is an integration API: the configured firing action authorizes the
launch and spends ammunition/heat in the same transaction. Historical shooter
identities do not prevent rounds from arriving after shooter removal.

The sparse `btech_artillery` table stores validated flight cursors with the world.
Map reload preserves the queue; map purge removes it. The one-second server tick
services queues even when every unit is shut down. Arrival updates, queue removal,
map/unit randomness, smoke, mines, damage and published character consequences use
the same commit boundary. Failure in a later round or in database persistence
restores the entire tick, allowing a single successful retry.

### Artillery catalogue and aiming

The typed catalogue now includes Inner Sphere/Clan Arrow IV, Long Tom, Sniper,
Thumper and their three cannon variants, with mass, ammunition capacity, heat,
critical requirements, Battle Value and recycle timing. Structurally valid artillery
launchers are admitted to live unit construction. Ordinary range and
instantaneous damage-group APIs reject artillery so it cannot silently use the
conventional firing pipeline.

`BattleWeapon::artillery_aim` accepts explicit live facts and computes the artillery
hit target without random draws. Maximum range is twenty times the catalogue long
range, including cannon entries in the reference. Extended range takes the larger
of that limit and twice medium range. Submerged mounts produce target 5000; raw
range overflow produces 1000, before skill or correction. In range, the base is
seven plus artillery gunnery: direct visibility subtracts two; indirect unassisted
fire adds one; a distinct observer adds `(spotting target - 4) / 2`,
truncated toward zero. A selected but unavailable observer adds neither term.
Stored correction is subtracted without a lower target-number clamp.

`unit_artillery_gunnery_target` always uses Gunnery-Artillery, independent of the
conventional extended-gunnery setting, and defaults to eight without an active
pilot. Configured firing supplies these facts and the saved correction when it
creates a queued flight.

### Artillery ammunition identities

Artillery now decodes the distinct `Cluster`, `Smoke` and `Mine` template flags
into typed ammunition identities for both bins and initial launcher selection.
`LBX/Cluster`, `Artemis/Mine` and `Narc/Smoke` retain their conventional meanings;
they are not aliases for artillery payloads. Conflicting artillery payload flags
are rejected rather than collapsed into a different round type. Standard,
cluster, smoke and mine selections map explicitly to delayed arrival effects.

Compact ammunition status uses C/S/M for artillery and retains L/A/N for the
corresponding conventional types. The LB-X toggle remains restricted to LB-X
weapons. Reference smoke/mine command eligibility is separate from artillery
payload decoding, so these types do not implicitly extend those commands.
Artillery launchers can fire their selected template payload through native and
Lua firing. The cluster control below switches between standard and cluster ammunition.


### Live artillery launches and correction

Native `fire <weapon>` and Lua `btech.unit.fire(unit, pilot, weapon)` dispatch
artillery to a dedicated coordinate launch action. Select a hex first, or use a
selected observer's hex lock. Explicit and selected unit targets are rejected.
Admission checks pilot authority, mechanical readiness and map availability.
Unassisted shots check weapon arcs, underground indirect-fire restrictions and
the configured observer requirement; observed shots use the separate path below. The shared
weapon launch action owns dice, ammunition, heat and recovery. A successful launch
adds one persistent flight in the same transaction; failed admission spends nothing.
Range and underwater sentinel targets produce misses, and artillery does not glance.

Lua returns `BattleArtilleryLaunchReport`, including aim, expenditure and the map
queue ordinal. Damage, smoke and mine effects are applied only on arrival. A miss
observed by a running friendly unit increments saved trajectory correction when
the shooter remains running and the original hex remains selected. A selected
observer receives correction feedback. Retargeting or changing observers clears
correction, including same-map units depending on the retargeting observer.

Tests compare native and Lua launches, permission/target/recycle rejection,
expenditure, delayed arrival, every payload on a hit, observed miss correction,
retarget reset and database replay both during flight and after arrival. Detailed
launch and arrival broadcasts are described below.


### Artillery cluster control

`cluster <selection>`, Lua `btech.unit.cluster(unit, pilot, weapon)` and
`toggle_battle_cluster` use the same authorization, intact-weapon, jam and recycle
guards as other weapon controls. Only artillery is eligible. The control toggles
normal/cluster selection and rejects an existing smoke or mine selection. It does
not spend ammunition or require matching supply to select a mode; firing still
requires a matching bin. Selection is saved with the unit.

Tests cover native/Lua equivalence, callback rollback of state and notices, pilot
rejection, normal-ammunition shortage without expenditure, saving selected mode,
live cluster arrival, recycle rejection and preservation of smoke/mine modes.
The original smoke/mine commands have distinct missile eligibility; they are not
artillery switching controls.


### Artillery launch and arrival broadcasts

Flights retain a typed artillery weapon rather than an arbitrary damage amount.
Construction and deserialization reject non-artillery weapons, and impact damage
comes from the catalogue. The same saved identity supplies arrival messages even
when the shooter has been removed or changed equipment.

Launch broadcasts distinguish Arrow missiles from other artillery rounds and name
the direction from the captured launch hex. Direction selection preserves the
reference's ordered ties and linear heading comparison. Arrival broadcasts name
standard fire, cluster surroundings, mine bomblets or smoke, with the weapon name
for standard and smoke rounds. Visible empty hexes generate messages too.

The arrival snapshots running viewers before damage or smoke changes visibility.
It labels the occupant's own hex and other coordinates with the matching emphasis,
while smoke uses plain text. Notices share the existing transaction and persistence
boundary with the effects; failed arrivals cannot leak their messages. Mine
explosions use the same hex-audience renderer.

Tests cover direction ties, launch feedback, all four arrival texts, occupied and
empty hexes, pending-removal/shut-down recipients, invalid weapon identities, saved weapon
identity and equal arrival reports/outboxes after database restart.


### Revalidated artillery observer links

Observed artillery shares live link validation with conventional indirect fire:
the observer must still declare spotting, be running and intact, remain friendly
on the same battlefield, and have a conscious pilot. Its selected hex must be
visible to its active sensors. Missing links or targets fail atomically; they do
not silently fall back to the shooter's selected hex.

Observed launches use indirect artillery aim even when the shooter sees the hex.
They follow the reference observer path rather than the unassisted arc and
underground admission checks. The shooter's unit lock and an observer's unit target
are rejected for artillery. Retargeting the observer clears its own correction and
that of same-map units following it.

Tests verify native/Lua equality, the indirect target number for a visible hex
behind the launcher, persistence, stale or stopped links, pending removal, absent
or wrong-kind targets, blocked observer sight, changed battlefields, rollback of
expenditure/notices and dependent correction reset. Blindness status itself
remains part of the broader sensor-state backlog.


### Artillery hotloading

Artillery supports the shared native/Lua hotload control and template flag while
retaining its separate aim rules and payload selection. Rolls of two or three jam
the launcher without ammunition, heat or a queued round; other rolls use ordinary
artillery launch expenditure. A jam or recycle timer prevents mode changes.

The first hotloaded artillery weapon critical uses catalogue damage when a live
ordinary ammunition bin exists. Special-only or empty supply does not explode,
and this critical check does not consume ammunition. Packet multiplication uses
a minimum count of one for weapons without missile racks.

Tests cover all eight families' hotload eligibility, template selection and saved
state, native/Lua launch and jam parity, expenditure, queued-round admission,
blocked mode changes, database replay and critical explosion eligibility.


### Ground-vehicle asset definitions

`BattleVehicleTemplate::parse` and `read_battle_vehicle_template` decode Vehicle
assets independently of BattleMech construction. Movement is typed as tracked,
wheeled, hover or stationary. Vehicle sections are left, right, front, rear and
an optional turret; aircraft and Mech sections are rejected. Hull faces require
positive internals. Equipment, modes, configuration and other fields remain
available for subsequent vehicle-specific validation rather than being discarded.

The shared brace/critical-range parser and bounded asset reader serve both Mech
and vehicle definitions. `cargo run --example btech_vehicle_audit -- game/mechs
Demolisher Flatbed_Truck Fulcrum RadioTower` prints decoded definitions and errors.
Successful decoding does not permit a vehicle to enter the Mech simulation. Live
vehicle construction, movement, damage, persistence and gameplay adapters remain
separate implementation work.


### Ground-vehicle equipment resolution

`BattleVehicleLoadout::resolve` interprets each vehicle weapon slot as a complete
weapon. Hull-face and slot order preserve weapon numbering. A Demolisher's two
adjacent AC/20 slots resolve to two weapons, without imposing Mech critical-slot
counts. Bins remain independent, with shared capacity, quantity and mode checks.

Weapon, ammunition and system records carry a typed location parameter so vehicle
faces cannot be mistaken for Mech limbs. Shared mount/bin constructors own mode,
one-shot, brand and supply validation; Mech allocation and split-critical rules
remain in the Mech resolver. Vehicle equipment resolution does not validate mass,
engine/system installation or admit a live vehicle to combat.

The audit resolves equipment for **220 of 235 vehicle assets** (222 parse).
RadioTower's Stinger ammunition is unsupported, and Svantovit-Streak has an
oversized ammunition bin. The other thirteen retain their parsing diagnostics.
All audit entries still report simulation support as false.

Tests cover the full existing weapon catalogue in single vehicle slots, artillery
payload/hotload selection, brands, systems, optional turrets, independent bins,
capacity rejection, unknown equipment, invalid modes and definition round-trip.


### Ground-vehicle engine diagnostics

`BattleVehicleTemplate::engine` reports nominal output, suspension-adjusted mass
rating, catalogue availability, powerplant family, engine mass and the hovercraft
minimum. Mechs and vehicles share speed-to-rating arithmetic, catalogue lookup
and fixed-point rounding. Vehicle rules apply fusion shielding before XL, XXL,
Light or Compact modifiers; combustion engines double base mass without shielding.
Conflicting technology flags retain the reference's precedence.

Tracked/stationary suspension is zero, wheels subtract twenty, and hovercraft use
the tonnage brackets 40/85/130/175/235. Hover engine mass is at least one fifth of
nominal tonnage. Missing catalogue ratings remain explicit in the report even
when the hover minimum supplies a mass; they do not establish valid construction.
Nonfinite, negative and overflowing input is rejected.

The vehicle audit includes these engine diagnostics. Tests cover real tracked,
wheeled and hover assets, technology precedence, shielding/rounding order,
suspension boundaries, missing ratings, hover minimums and saved definition replay.
Complete vehicle mass, structural validation and live simulation remain unfinished.

### Intact ground-vehicle mass diagnostics

`BattleVehicleTemplate::mass()` resolves equipment and returns `BattleVehicleMass`,
with component values in 1/1024-ton units. It accounts for installed engine mass
(including shielding and the hover minimum), quarter-ton controls and turret
rounding, hover components, structure and armor technology, whole vehicle weapons
and systems, cooling beyond engine capacity, cargo space, and ammunition.

The report distinguishes `total`, which uses the rounds listed in the asset, from
`design_total`, which charges installed full- or half-ton bins. It does not refill
or normalize the source ammunition. `ammunition_capacity` is bin mass, not a round
count. Cooling uses the declared sink count; omitted or zero fusion cooling defaults
to ten, and omitted combustion cooling to zero. Vehicle sinks occupy one slot,
with Clan/DoubleHS efficiency applied before subtracting ten integral fusion sinks.
Cargo defaults to zero when absent and uses Carrier, CargoTech, or standard space
conversion in that precedence order.

The vehicle audit example includes this mass report or a contextual error alongside
its engine and equipment diagnostics. Missing engine catalogue ratings, invalid
cargo, unresolved equipment, and incomplete hull anatomy prevent a mass report.
A successful report is not construction approval or live vehicle simulation support;
movement, damage, systems installation legality, and command integration remain
separate work. Vehicle XXL engines use the asset flag `XXL_Tech`.

### Ground-vehicle material state

`BattleVehicle` owns its template, current protection, and current ammunition.
Construction requires resolved equipment and a complete intact mass calculation.
Loadouts remain derived; snapshots do not duplicate equipment definitions.
Deserialization validates section membership, original protection limits, bin
counts and capacities, and the absence of armor or ammunition in destroyed sections.

`damage_phase` uses the shared `BattleDamagePhase` and a section-typed
`BattleDamageResult<BattleVehicleSection>`. Armor and internals are separate phases
so a combat caller can insert critical resolution between them. Vehicle hits use
the selected face's armor; the Mech rear-armor selector does not redirect them.
Section loss clears its protection and ammunition. Any hull-face loss destroys
the vehicle; turret loss alone does not. Overflow returns to the caller and does
not implicitly transfer into another face. Checked ammunition expenditure and
invalid-section damage calls leave state unchanged on failure.

This is domain state with validated JSON replay. It is not yet connected to world
membership, SQLite vehicle storage, movement, hit-location selection, critical
resolution, crew effects, or live vehicle commands.

### Ground-vehicle world ownership and saved games

`create_battle_vehicle` registers an owned `BattleVehicle` on an unused live thing.
`BtechState::vehicles()` exposes vehicle records separately from the Mech simulator,
while `units()` includes their common identities. Vehicles retain the shared MECH
special-object registration, class code 1, and their ground movement identity.
`damage_battle_vehicle_phase` applies a material phase to a world candidate and
rejects unavailable objects; combat effects remain the caller's responsibility.

The optional `btech_vehicles` SQLite extension stores bounded, versioned snapshots
inside the enclosing world transaction. A failed registration write rolls back the
vehicle write too. Loading validates snapshot contents, identity ownership, and
registration conflicts. Purging the owning object removes vehicle state, common
identity, and registration together, including when foreign-key cleanup is disabled.
Templates are owned by the record rather than reloaded from the game directory.

Vehicles remain unplaced until battlefield admission is implemented. Mech movement
and combat iteration do not accidentally activate these vehicle records. Vehicle
commands, placement, movement and the complete combat pipeline remain unfinished.

### Ground-vehicle administrative placement

The shared `place_battle_unit` and `remove_battle_unit` entry points now dispatch
to vehicle placement when the object owns a vehicle. Placement validates decoded
terrain, coordinates, live containers and containment cycles before changing state.
Position, shared map slot, identity and object location are published together;
protection and ammunition remain unchanged.

Mechs and vehicles allocate from one battlefield slot namespace. Same-map placement
retains the slot, and new arrivals reuse the first vacancy. Vehicle positions and
slots survive SQLite replay. World validation rejects mismatched containment,
missing maps and duplicate mixed-class slots. Occupied maps cannot be reloaded;
map purge clears surviving vehicle positions and slots alongside common identities.

This is administrative placement, not vehicle locomotion or combat admission.
Mech-only combat iteration still excludes vehicles until its consumers support them.

### Vehicle administration through native commands and Lua

`@btech unit-create` and `btech.unit.create` now select construction rules from the
asset's declared Type. `BattleUnitTemplate` and `read_battle_unit_template` provide
the same checked dispatch to Rust callers. Asset reads retain directory confinement
and the one-megabyte bound. Malformed Mech definitions never fall back to vehicle
parsing, and unsupported unit classes are rejected explicitly.

`btech.unit.state` returns a detached class-specific projection. Its `kind` is
`mech` or `vehicle`; vehicle state includes definition, protection, ammunition,
position, battlefield slot, destruction state and `simulation_supported=false`.
It does not fabricate Mech-only power, pilot or motion fields. Existing placement
and removal entry points work for created vehicles, with callback rollback covering
both the vehicle record and object containment. Vehicle commands for movement and
combat remain pending.

### Ground-vehicle cockpit ownership and power

Vehicles now use the existing `pilot`/`unpilot`, `startup`/`shutdown` commands and
Lua unit pilot/start/stop operations. Assignment requires a live player physically
inside an available vehicle, rejects an occupied cockpit, and shares the single-unit
pilot invariant with Mechs. Ordinary player departure and player purge release the
assignment. Shutdown also releases it; administrative relocation clears it.

Normal startup takes thirty committed seconds, with movement-specific messages at
five-second intervals. Wizard override takes five seconds. Vehicle startup timers
participate in the server's active-work check and ordinary transactional heartbeat:
failed saves restore the countdown and discard notices. Snapshots validate countdown
bounds, placement for powered vehicles, and shutdown of destroyed hulls. Hull loss
and map purge shut down vehicles; administrative placement requires shutdown.

This exposes the power lifecycle only. Vehicle motion, scanner/contact activation,
startup observer broadcasts and combat are still pending; the vehicle Lua projection
continues to report `simulation_supported=false` and now includes pilot and power.

### Ground-vehicle motion proposals

`BattleVehicleTemplate::ground_motion_step` computes a proposed one-second motion
segment from an intact chassis, current terrain, a `BattleMotion`, and explicit
`BattleVehicleMotionRules`. Turning and acceleration share the Mech implementation.
The vehicle calculation applies rough/snow/forest/mountain speed costs, tracked and
wheeled road/bridge bonuses, reverse speed limits, turning slowdown, Speed Demon
acceleration, map movement scaling, and paved-surface turning above nominal speed.
Stationary chassis reject active motion. Nonfinite or out-of-envelope inputs fail.

The result is a proposal only: it does not change vehicle/world state or imply
that its destination is traversable. Terrain transitions, collisions, mines, water,
cliffs, crew checks and notifications must be resolved by a vehicle movement adapter
before committing a proposed segment. Live vehicle movement remains unfinished.

### Live ground-vehicle travel on supported terrain

Vehicles now retain `BattleMotion` with their saved placement. Existing speed and
heading controls, named speeds and cockpit readouts dispatch to vehicles. Controls
require a conscious assigned pilot in a running vehicle; readouts remain available
to conscious occupants. Shutdown and hull destruction halt motion, and placement
starts a new stationary motion record. Snapshots reject inconsistent motion/hex
coordinates, active motion while powered off, and invalid throttle values.

Vehicle travel participates in the shared movement candidate and server heartbeat.
Each proposed segment is traced through crossed hexes before committing coordinates.
Continuous and hex positions replay together; battlefield slots and containment do
not change during travel. Failed movement/save transactions roll back with the world.

Ground travel covers ordinary land, forests, rough terrain, bridge decks, hovercraft
underpasses and water entry. Elevation changes, cliffs, reverse slopes, mines,
water destruction and low bridge collisions apply their movement consequences.
Occupied hexes do not block vehicle travel. Tracked/wheeled ice entry uses the shared fracture check; remaining terrain
types still have explicit admission boundaries. Character fall/flooding
casualties remain unfinished.
The Lua `simulation_supported=false` marker continues to indicate incomplete vehicle
simulation, even though ordinary ground travel is now active.

### Hovercraft water surfaces and shared geometry

Hovercraft can now cross normal water of varying depth and enter or leave a
level-zero shoreline without treating the bed depth as a slope. Vehicle support
height is class-aware, and `battle_unit_elevation`, mixed-class `battle_unit_range`,
and Lua vehicle state use that same height. A hovercraft above deep water remains
at elevation zero; a submerged Mech's range contribution still uses its own depth.

Driving now uses underlying terrain for speed costs and support height while
checking fire overlays separately. Smoke cannot disguise water or erase forest
speed costs. Level mountain terrain is admitted with its existing speed penalty.
Tests cover deep-water travel and shoreline exit, smoke, persistent motion replay,
mixed Mech/vehicle spatial range, and wheeled rejection of unsupported water
under smoke. Vehicle flooding, collisions and remaining combat consequences
are still unfinished.

### Vehicle one-level slopes

Vehicle path tracing now compares each crossed hex with the preceding support
height, admitting ordinary one-level climbs and descents. Each elevation change
reduces speed toward zero by two movement points (21.5 kph), preserving the requested
throttle so acceleration can resume. Water-depth changes under hovercraft remain
level travel. Larger changes use a shared speed-based cliff avoidance check.
Success stops before entry; failed uphill checks apply speed-based fall damage
and restore the previous position. Failed drops leave the vehicle below. Skid
rules retain the zero-damage vehicle crash while still checking pilot injury
and fall-sensitive mines. Tracked/wheeled drops into water flood survivors unless
the template has Waterproof_Tech. Cliff events bypass ordinary step effects.

Tracked vehicles can reverse across ordinary slopes. Wheeled and hover vehicles
use the same reverse-slope control check as Mechs. Failed checks apply vehicle
falls: uphill failures restore the previous position, while downhill failures
remain below. Enabled vehicle reverse checks bypass ordinary entry mines, fire
checks and slope speed loss, even on success. Pilotless vehicles pass without
rolling. Disabled checks and tracked travel retain ordinary step effects. Tests
cover success and both fall directions, exact successful dice consumption,
restart replay, configuration, speed loss and cliff stops.

### Vehicle bridge decks and hovercraft underpasses

Ground vehicles can cross bridge decks using ordinary slope limits and speed
loss. Hovercraft arriving from water can pass beneath spans at least two levels
high, maintaining elevation zero across changes in deck height. Leaving the span
clears the saved under-bridge state. Placement starts a vehicle on the deck.

Underpass state survives SQLite replay and is checked against vehicle class and
map terrain. Low spans now use shared terrain control checks. Success stops before
impact; failure applies a one-level vehicle fall. Both restore the previous
under-span position and stop controls, preserving any damage and heading change.
Occupied hexes still stop travel, including units at different bridge heights,
until vehicle collision rules are implemented. Tests cover deck travel, varying
span heights, water exits, replay and invalid saved underpass state.

### Hovercraft ice travel

Hovercraft now cross intact ice at elevation zero, including varying underlying
depths, water/shore transitions and passage beneath clear bridges. Movement does
not trigger ice fracture for hovercraft, matching the class exemption in the
reference rules. Smoke does not change the supporting terrain. Tests check intact
terrain after travel, full speed, shared geometry and SQLite replay on ice.
Tracked and wheeled ice movement remains stopped pending fracture and casualty
effects; this admission applies only to hovercraft.

### Private vehicle dice

Vehicles own independent replayable dice streams using the same tagged generator
as Mechs. The shared domain dice operation supports both classes, validates its
bounded request before consuming outcomes, and commits vehicle outcomes in the
vehicle snapshot. Lua state and debug output do not reveal the stream.

Tests exercise restart continuation, failed-save rollback and retry, independent
streams, invalid requests and unknown algorithm rejection. Vehicle clearance, cliff, reverse-slope and water checks consume the same saved
stream and apply their consequences through the movement transaction.

### Vehicle control checks

The shared piloting-target and control-check APIs now accept ground vehicles.
Basic skill mode uses Drive; extended mode selects tracked, wheeled or hover
piloting. Fixed installations use the default target in extended mode. Present
connected pilots use saved character skills and earned levels; other crews use
target six. Small cockpits and missing in-character operators add their penalties.

Checks consume the vehicle's saved dice only when powered and conscious, and
reject unavailable objects before any mutation. Tests cover each skill mode,
modifiers, extreme targets, crew restrictions and SQLite replay. Vehicle mobility
criticals and blindness are not yet represented, so their modifiers remain absent.
These domain checks do not apply XP, notices or failure effects; movement hazard
adapters still need to combine those effects with the check in one transaction.

### Standard vehicle hit locations

`BattleVehicle::standard_hit` resolves an existing 2d6 location roll against the
standard ground-vehicle table. Hull faces use attack direction; working turrets
receive the table's turret rolls, while missing or destroyed turrets redirect
those hits to the hull. The result identifies the face and any through-armor
critical candidate without changing protection or selecting damaged components.

Secondary dice follow strict hull/turret armor thresholds, the full-armor chance,
vehicle-critical mode and originally unarmored faces. Stationary and critical-proof
vehicles skip critical eligibility. Tests cover every direction/roll with working,
destroyed and absent turrets, threshold boundaries, exemptions and dice order.
FASA/advanced tables, dug-in/combat-safe routing, applied criticals and the live
vehicle attack transaction remain unfinished.

### FASA vehicle hit effects

`BattleVehicle::fasa_hit` returns a hit location with explicit motive-speed loss,
immobilization and turret-lock effects. Inputs separate configuration from existing
immobilization/turret-lock conditions. Critical-proof equipment takes precedence.
The resolver preserves front/rear shielding policy, friendly criticals, side-specific
hovercraft behavior and configured roll-twelve critical thresholds.

Tests cover every roll/direction across policy combinations, existing damage
conditions, turret loss, critical-proof equipment and secondary RNG order. These
results still require a damage transaction to persist and apply their effects.
Advanced tables, dug-in/combat-safe routing and live vehicle combat remain pending.

### Persistent vehicle motive damage

Vehicles now retain motive speed loss and immobilization independently of their
construction definition. Applying a motive hit clamps actual and desired speed to
the reduced forward/reverse envelope; exhausting speed or immobilization halts
translation and turning. Ordinary movement, acceleration, heading controls and
native/Lua throttle requests use the current maximum. Lua state exposes that
maximum, speed loss and immobilization.

SQLite replay preserves damage, and snapshot validation rejects impossible loss
values or motion outside the damaged envelope. Tests cover forward/reverse limits,
restart movement, Lua controls, immobility and invalid snapshots. The domain motive
operation is ready for a damage transaction; attack adapters, notices, turret-lock
state and other vehicle critical effects remain unfinished.

### Persistent vehicle turret controls

Ground vehicles with surviving turrets now support the native `turret` command
and Lua `btech.unit.turret`. The assigned conscious pilot can query or set an
integer absolute heading while powered. Facing is stored relative to the hull,
so hull turning also turns the turret. Immobilized hulls retain turret control.

Turret locks persist, reject controls and feed the current hit-condition query.
Turret destruction clears the lock and facing; snapshot validation rejects
missing turrets with residual state. Tests cover native/Lua parity, callback
rollback, saved replay, damage locks, turret loss and invalid headings/snapshots.
Jamming, automatic target tracking, turret weapon arcs and combat invocation of
lock effects remain unfinished.

### Vehicle weapon arc geometry

Vehicle weapon mounts now test their own hull-face arcs or a 60-degree turret arc
against compass bearings. Geometry follows the reference's whole-degree facing
and rounded target bearing. Rear-mount flags do not override a vehicle hull face.
`BattleVehicle::weapon_bears_on` resolves a saved weapon index, current hull/turret
facing and section survival; a locked turret remains usable within its fixed arc.

Tests cover boundary angles, wraparound, rounding, hull turns with a locked turret,
SQLite replay and lost/unplaced weapon rejection. This is a geometry query, not
a firing action: targeting, ammo, recycle, power and damage transactions remain
separate vehicle-combat work.

### Vehicle gunnery skill lookup

The shared unit gunnery query now resolves vehicle loadouts. Basic gunnery uses
Gunnery-Conventional; extended gunnery uses the existing weapon-family catalogue.
Present connected pilots contribute saved character skills and earned levels;
missing/disconnected pilots use target six. Invalid weapon indices still fail
before fallback. Queries do not consume dice, grant XP or establish firing authority.

Tests cover basic versus family skills, energy/flamer/ballistic/missile/artillery
weapons, signed targets, saved values and absent crew. Separate gunner assignments,
vehicle firing and its XP/recycle/damage transaction remain unfinished.

### Vehicle weapon reservations and recycling

Vehicles now persist per-weapon recycle timers and expended one-shot launchers.
Mechanical readiness checks power, hull/section survival, matching ammunition,
launcher expenditure and recycling. Normal-cycle reservations prefer a compatible
bin in the weapon's section, then canonical bin order; energy and one-shot weapons
do not draw external ammunition. Invalid or unavailable cycles leave state unchanged.

The reservation accepts the caller's launch decision. A failed Streak lock starts
the normal recycle timer without consuming ammunition or a one-shot charge; other
weapons cannot reserve an unlaunched cycle. Ground vehicles do not use live heat.
Non-normal fire modes and defensive AMS use are rejected. Targeting, Streak lock
resolution, jams and the complete attack outcome remain unfinished.

Powered vehicle timers now advance in the shared server tick, including idle
vehicles. Shutdown pauses timers, lost sections discard their timers, and SQLite
replay preserves countdown and expenditure. Tests cover normal/energy/one-shot
cycles, shortage, invalid snapshots and a real idle-server failed-save retry.

### Vehicle movement contributions to aim

Vehicle shooter movement now shares ground attack arithmetic with Mechs: stopped,
cruising and flank speeds, optional FASA hull-turn penalties, and the original
construction walking threshold after motive damage. Turret rotation adds no hull
movement penalty. Reverse movement retains the cruising penalty.

The shared target-movement query now accepts vehicles and is also used by Mech
weapon aim. It combines absolute speed bands with shutdown, unconscious crew,
stationary construction or immobilized motive state. Merely stopping, or reducing
the maximum speed to zero through incremental losses, does not imply the explicit
immobilized state. Read-only queries validate live objects and finite nonnegative
ranges. Tests cover boundaries, turning, reverse motion, damage, crew, restart and
stationary construction. Vehicle sensor acquisition and the full aim/attack action
remain unfinished; these arithmetic queries do not grant firing permission.

### Stationary construction without propulsion

Stationary vehicles with a zero propulsion rating now produce complete mass
reports and can be created, powered and saved. Their engine diagnostic retains
an absent catalogue entry and zero installed propulsion mass. Positive ratings
and mobile chassis still require a catalogue entry. Tests verify that stationary
units reject driving, retain turret controls and weapon recycling, and replay
through SQLite. The original RadioTower asset still requires Stinger ammunition
support; its missing propulsion is no longer a construction admission failure.

### Stinger ammunition and RadioTower admission

Stinger is now a typed ammunition mode for supported indirect missile families.
Templates preserve selected launchers and separate bins, ordinary capacities and
missile damage groups; compact status uses `T`, and Lua state types include the
mode. Range calculations extend maximum reach by seven hexes while retaining
minimum penalties and rounded intermediate brackets. Command-network range
sharing retains its existing long-range limit.

Direct shots reject grounded targets and coordinate fire before expenditure or
dice changes. Jumping Mechs are eligible through the ordinary optical and firing
checks; save/reload reproduces the complete attack. Hex aim rejects Stinger shots.
Flying aerospace/VTOL and orbital-drop target states, their accuracy bonuses, and
native/Lua Stinger selection controls still need implementation.

The unchanged RadioTower asset now resolves its Stinger equipment and zero-rated
propulsion and can be created, powered, controlled and saved. Its turret and
ordinary weapon recycling are covered by the stationary integration test. Complete
vehicle firing remains unfinished; vehicle reservation callers own attack admission.

### Stinger selection controls

The `stinger` command and `btech.unit.stinger` now share controlled, intact,
recycled-launcher checks and one-shot rejection. Selection replaces another
ammunition mode, a second toggle returns to normal, and matching stock is required
for readiness rather than selection. Native/Lua parity, invalid controls,
empty matching supply, callback rollback and SQLite replay are tested. These
controls currently apply to constructed Mechs; vehicle mode controls and complete
vehicle attack integration remain unfinished.

### Vehicle ammunition selection

Vehicles now own validated per-weapon ammunition selections, initialized from
mount flags and persisted separately from construction. Normal selections are
implicit. Readiness and reservations use live selections to choose matching bins,
so changing back to normal from a template-selected Stinger launcher works across
restart. Lua unit state exposes the selected modes.

The existing `stinger` command and Lua operation now accept vehicles, including
stationary turrets. They require a conscious assigned operator, running unit,
intact recycled launcher and mutable (not one-shot) ammunition mode. Tests cover
native/Lua parity, failed callback rollback, restart, normal versus Stinger bin
expenditure, empty matching supply and invalid saved selections. Other vehicle
mode commands and complete attack admission/resolution remain unfinished.

### Shared vehicle ammunition controls

Vehicle and Mech ammunition controls now share authorization/readiness dispatch
and mode storage updates. This removes the separate Stinger vehicle toggle and
extends `lbx`, `sguided`, `precision`, `flechette`, `armorpiercing`, `caseless` and
`incendiary`, plus their Lua equivalents, to vehicles. Weapon-family restrictions
remain in their individual controls, and missile controls retain one-shot checks.

Tests exercise every added vehicle mode through native and Lua commands, matching
ammunition readiness/expenditure, callback rollback, persistence and rejection
while recycling. These are selection and reservation changes; full vehicle
attacks must still apply each selected round's effects and loader failures.

### Individual vehicle equipment losses

Vehicles now persist explicit lost equipment slots separately from construction.
Weapon readiness, firing arcs and ammunition supply account for those losses.
Destroying a weapon cancels its countdown; destroying a bin removes its rounds.
Other equipment and section protection remain intact. Repeated loss is idempotent,
and snapshots reject nonexistent slots, ammunition in destroyed bins and recycle
timers on destroyed weapons. Lua unit state exposes the lost locations.

The explicit damage primitive does not choose criticals, resolve ammunition
explosions, apply system-specific consequences or publish notices. Those remain
part of the complete vehicle damage transaction. Tests cover individual weapons,
matching supply, unchanged construction, invalid input, unavailable objects,
snapshot rejection and deterministic reservation replay after restart.

### Vehicle weapon critical selection

Section-local weapon critical selection now filters destroyed mounts and sections,
then chooses uniformly in stable slot order using the victim vehicle's saved dice
stream. Empty candidate sets consume no dice; a single remaining candidate still
consumes a selection roll. Empty ammunition, spent charges and recycling do not
protect otherwise intact mounts.

The selection is deliberately separate from applying weapon-specific consequences:
callers must handle explosive equipment and publish damage within the enclosing
transaction. Tests compose selection with ordinary mount destruction and verify
slot exclusion, no-effect dice preservation, exact stream advancement and replay
through SQLite. Complete critical-table dispatch remains unfinished.

### Ground-vehicle critical table reports

Ground vehicles now have standard, FASA and advanced critical-table selection.
Advanced configuration takes precedence. The advanced table maps rolls six through
twelve by hull face/turret; lower rolls have no effect. Standard criticals first
check section-specific motive/turret outcomes, respecting existing immobilization
or turret locks, then fall through to the six-entry severe table. FASA goes
directly to that severe table.

Explicit rule inputs suppress disabled, combat-safe and critproof units before
rolling. Stationary units consume no standard/FASA dice; the advanced table rolls
2d6 before its stationary check. Reports retain table, section, actual draw totals
and selected effect. Victim-owned dice commit with the caller's world transaction.
Tests cover all advanced outcomes, standard/FASA branches, suppression precedence,
exact stream consumption and SQLite replay.

These reports select consequences without applying them. Table-specific engine
behavior, fuel technology fallback, containment, crew effects, equipment selection
and notifications still require the complete damage transaction. Ground-vehicle
reports do not stand in for naval, VTOL or aerospace critical tables.

### Vehicle driver, sensor and stabilizer damage

Vehicles now persist cumulative handling and sensor penalties plus section-local
stabilizer losses. Driver hits add two to live piloting checks; sensor hits add one
to the shooter control contribution. Destroyed stabilizers double the movement
contribution only for weapons in that section, including FASA turning, without
doubling sensor penalties. Repeated stabilizer hits do not stack. Cumulative
penalties saturate at 127, preserving the reference signed-byte ceiling.

Construction and character skills remain unchanged. Lua state exposes the damage.
Tests cover live piloting, independent hull/turret modifiers, stationary turning,
flank movement, idempotence, saturation, invalid snapshots and saved replay.
Critical-table application and full vehicle aim/attack integration remain open;
commander and crew-stun consequences are supported below.


### Vehicle crew stun and commander damage

Crew stun lasts 60 simulation seconds. Repeated hits restart that countdown,
including while powered off. Stun reduces an excessive forward throttle to cruise
minus 0.1 kph and prevents flank-speed requests and weapon reservations. Reverse,
heading and turret controls remain available. Hull destruction cancels recovery.
Commander hits add one piloting and one gunnery penalty and stun the crew. The
shared firing penalty is exposed as `gunnery_damage`; `crew_stun_remaining` is
available in Lua state and saved with the vehicle.

Tests cover restart, repeated hits, control admission, destruction and recovery
while powered off, including rollback and retry after a failed server save.
Critical-table dispatch and full vehicle targeting and firing remain unfinished.


### Vehicle temporary weapon critical failures

Section-local weapon-jam criticals select an intact weapon that has no existing
failure and disable it for 60–120 powered seconds. Selection and duration use
the saved vehicle dice stream; sections with no candidates consume no dice.
Ballistic weapons jam and other families short out. A critical replaces any
ordinary recycle countdown, without spending ammunition or destroying equipment.

Recovery uses the existing weapon timer, pauses while shut down, and reports
that the weapon is operational again. Individual mount or section loss removes
the temporary failure. Lua exposes `weapon_failures` beside `weapon_recycle`.
Tests cover selection, repeat-hit exclusion, exact dice advancement, saved replay,
shutdown pauses, recovery, invalid snapshots and destruction cleanup.
The older main-weapon-jam critical, turret jams and automatic critical dispatch
remain separate unfinished work.


### Vehicle turret jams and crew repair

Turret jams preserve facing and firing arcs while blocking rotation. A second
jam becomes a permanent lock; directly locking a jammed turret clears the jam.
Native `fixturret` and Lua `btech.unit.fixturret(unit, pilot)` start 60-second
attempts. Each request schedules an independent attempt (up to 256 pending).
Firing remains blocked until every attempt has elapsed. Timers continue while
powered off; completion requires a running vehicle and conscious current pilot,
and cannot remove a permanent lock. Turret or hull destruction cancels attempts.

Tests cover repeat attempts, restart replay, failed completion after shutdown,
lock promotion, firing admission, invalid snapshots, native/Lua rollback and
destruction cleanup. Full critical dispatch remains pending.


### Atomic vehicle critical resolution

`resolve_battle_vehicle_critical` joins table selection with control, motive,
engine, temporary weapon-jam and turret consequences in a private world candidate.
Successful results carry the original rolls and occupant notices for the enclosing
attack. Errors publish neither dice consumption nor partial damage. Cargo results
retain the reference's no-effect notice.

Standard/FASA engine hits remove propulsion without setting the advanced
immobility flag. Advanced engine hits immobilize; advanced fusion-engine fuel
hits use that same effect. Combustion fuel explosions remain unsupported and
return an error, as do crew casualties, explosive weapon losses,
ammunition explosions, power-plant explosions and turret blow-off. Callers must
abort an attack on these errors, never reinterpret them as harmless hits.

Tests exercise all three tables, state changes, saved replay, suppression and
unsupported-outcome rollback, including the fusion/combustion distinction.
The full vehicle attack path still needs the remaining consequences and targeting.


### Standard and FASA main-weapon jams

Main-weapon criticals rank every intact mount in hull-face/slot order using one
31-bit draw per weapon, selecting the highest positive value. Existing failures,
recycling and empty ammunition do not exclude a mount. The selected weapon is
marked `disabled` without spending ammunition, changing construction or adding
a timer. Section-local temporary jams continue to exclude every failed mount.

The reference has recycle-dependent recovery: a disabled weapon with no timer
remains unavailable, while one already recycling recovers on its next powered
recycle update. Shutdown pauses that update. Both cases persist across restart;
physical mount/section loss removes the failure. Firing and ammunition controls
reject a disabled mount. The atomic critical dispatcher now handles this outcome.

Tests cover ranking and exact dice advancement, existing failures, no-candidate
suppression, persistent disablement, powered recovery, saved replay and cleanup.
Crew casualties and explosive critical consequences remain unfinished.


### Tactical vehicle crew casualties

Vehicles now use the shared tactical pilot-injury and consciousness path. Injury
counts persist with the vehicle; recovery dice and 30-second retry timers remain
player-owned. Six injuries cause scenario loss, shutdown and cockpit release
without changing armor or construction or killing the player object. Active crew
stun and turret-repair attempts are cancelled. Destroyed vehicles reject cockpit
assignment.

The critical dispatcher applies crew-hit and crew-killed outcomes for a present
assigned tactical pilot and returns the injury report with staged notices. Its
rules explicitly select toughness checks. In-character casualties and absent
pilots remain unsupported: the complete critical candidate, including its roll,
is discarded on error. Those cases need the character/automated-crew lifecycle.

Tests cover injury and recovery replay, scenario death, unchanged material and
player state, timer cancellation, corrupted saves and casualty admission rollback.


### Vehicle ammunition cascade preparation

Vehicle ammunition criticals can now inspect and atomically empty the cascade's
bins within an enclosing attack candidate. The report retains the hit section,
exact ammunition draws and total pending internal damage. All live non-Gauss
bins across the vehicle participate, independent of ammunition mode; plasma
rounds participate here despite being inert in ordinary bin explosions. Gauss
rounds, empty bins and destroyed equipment do not participate. A zero-damage
report requires the critical's weapon-destruction fallback.

This prepares the ammunition consequence only: the caller must resolve internal
damage, resulting criticals, casualties and visibility notices before committing.
The critical dispatcher now consumes this preparation through the atomic
internal-damage resolver described below. Tests cover explicit mixed-bin damage totals, loss
exclusion, unchanged construction, invalid victims and persisted replay.


### Internal vehicle explosion resolution

Internal damage now rolls secondary criticals before reducing structure, including
reinforced/composite structure rounding. Reports retain damage-entry and critical
rolls, nested critical results, ammunition draws, discarded excess and staged
occupant/visibility notices. Vehicle-local internal explosions do not transfer
excess to other faces. Resolution stops further secondary criticals once the
vehicle or hit section is lost and bounds nested cascades to 64 levels.

The critical dispatcher now resolves ammunition cascades, empty-ammunition weapon
loss, ordinary weapon destruction, Gauss explosions and turret blow-off. Explosive
mounts and ammunition are disabled/emptied before secondary criticals, preventing
repeat explosions. Disabled Gauss weapons do not explode. Surviving tactical
pilots take explosion injuries through the shared injury path.

Nested unsupported effects roll back the entire candidate, including spent bins
and all dice. Fatal in-character damage and associated character/evacuation handling remain
unsupported.
The enclosing attack still owns publication of visibility-filtered broadcasts;
these APIs do not make the full vehicle targeting/firing path complete.

Tests cover structure rounding, critical ordering, whole-cascade rollback,
surviving Gauss injuries, disabled-weapon behavior, ammunition overflow remaining
local to the turret, and saved replay.


### Tactical fuel and power-plant explosions

Fuel and power-plant criticals now destroy the victim's sections atomically.
These catastrophes do not apply an area blast to nearby battlefield units. FASA
power-plant explosions with installed CASE destroy only the rear section;
standard power-plant and fuel explosions destroy all remaining sections. Clan
technology and configuration text alone do not install the CASE equipment used
by this check. Advanced fusion fuel hits continue to resolve as engine damage.

Section loss now releases the cockpit when the vehicle is destroyed. Unaffected
sections and their ammunition remain intact after a contained catastrophe; full
explosions clear all section ammunition, weapon timers and turret damage. Tactical
player objects remain in the wreck and are not killed as RPG characters.

The complete candidate is rejected for in-character catastrophes or an
uncontained explosion carrying battlesuits, pending the appropriate casualty
lifecycle. Reports retain section losses and staged occupant/visibility notices.
Tests cover table-specific containment, exact dice advancement, neighboring-unit
isolation, saved replay and casualty rollback. Full attack publication, character
casualties and unsupported unit classes remain unfinished.


### Vehicle hotloaded and incendiary weapon criticals

Vehicle weapon destruction now resolves hotloaded launcher explosions when a
usable normal-ammunition bin remains anywhere on the vehicle. The damage equals
a full salvo. Incendiary ammunition ignites for the weapon's damage only while
the selected weapon is recycling and usable incendiary supply remains. These
checks use live bin contents, individual equipment losses and the live ammunition
selection. Neither explosion spends additional ammunition nor directly injures
the pilot; secondary criticals still apply their own crew effects. Disabled
weapons do not explode. Other firing modes permit ordinary weapon destruction.

The mount is disabled before internal damage and nested criticals, within the
same atomic candidate. Save/reload preserves both eligibility and outcomes.
Complete vehicle attacks remain unfinished.


### Live vehicle firing-mode controls

Vehicles now own persistent firing-mode selections initialized from their
construction templates. Native and Lua heat, hotload, Ultra, rapid-fire, gatling
and rotary controls share the existing equipment rules and require a conscious,
present assigned pilot, running power and an intact, recycled weapon. Hotload
still rejects one-shot launchers. Empty ammunition does not block mode selection.
Repeated rotary selection retains the selected burst; the other controls toggle.

Normal is implicit in saved state. Loading an empty selection never reapplies
non-normal template flags. Snapshot validation shares weapon-family eligibility
with Mechs and rejects explicit normal, unsupported modes and invalid indices.
Lua vehicle state exposes the non-normal `fire_modes` map. Critical explosions
and normal-cycle reservations consult the live mode. Vehicle firing reservations return their effective mode for the enclosing attack.

Tests exercise all six native/Lua controls, notification and state rollback,
saved selections, cockpit guards, snapshot rejection, one-shot restrictions and
live hotload changes affecting critical damage after restart.


### Vehicle mode-aware firing reservations

Vehicle ammunition feeds now share draw ordering with Mechs: use matching live
bins in the mount section first, then canonical section/slot order, skipping lost
and empty bins. Read-only plans expose short supply without consuming ammunition
or dice. Ultra, rapid-fire and rotary reservations draw the requested burst
across bins, falling back to one normal shot and saving that selection when
supply is insufficient. Heat and hotloaded cycles retain their selected modes.

Gatling preparation consumes one saved d6 before attack dice, caps its damage by
available ammunition triples, and preserves the one-point floor when fewer than
three rounds remain. Reservations return effective firing mode and gatling damage
alongside actual ammunition draws. Ground vehicles do not accumulate live heat.
Rejected reservations preserve supply, mode and dice; failed Streak locks still
recycle without consuming ammunition. The enclosing attack must apply targeting,
hit resolution, mode-specific failures and damage before publishing the cycle.
Complete vehicle attacks remain unfinished.

Tests cover all burst sizes, local-bin priority, cross-bin draws, saved fallback,
gatling supply boundaries and exact dice progression, heat without ammunition,
rejected cycles and persistence replay.


### Vehicle armor-to-structure resolution

`resolve_battle_vehicle_armor_damage` applies an already located hit inside one
candidate: hardened armor rounds damage up to half, armor absorbs protection,
through-armor/AP criticals resolve, then any overflow reaches internal structure.
AP checks require no penetration and less than 50 percent of original armor
remaining, with the existing autocannon-size roll penalty. A hit-table critical
takes precedence over the AP adjustment. Combat-safe hits consume only their
entry roll; zero damage is a no-op after input validation.

Penetration reuses the internal-damage resolver without another damage-entry roll
or duplicate hit notice. Its internal critical roll is always consumed, but
additional critical dispatch is suppressed when the armor stage already produced
critical counts. Reinforced/composite structure, nested explosions, crew effects,
section loss and tactical destruction remain part of the same candidate. Excess
damage remains local to the vehicle section. Unsupported casualties roll back
armor, internal damage, equipment, crew changes and random streams together.

The caller still owns hit-table motive effects, searchlights, armor warnings,
weapon-specific damage adjustments and visibility/attack publication. This is a
material-resolution stage, not yet a complete vehicle attack command. Tests cover
material rounding, armor-only and penetrating dice order, critical suppression,
AP thresholds, tactical turret/hull loss, combat safety and casualty rollback.


### Atomic vehicle hit-table impacts

`resolve_battle_vehicle_impact` resolves one positive, already successful damage
group from its incoming arc. Standard, FASA and advanced hit tables now feed the
same armor/internal resolver. The candidate includes hit-routing and conditional
dice, motive speed loss, steering penalties, turret locking, protection loss,
nested criticals and feedback. Any unsupported later consequence discards all
of those changes. Combat-safe routing preserves material state and consumes the
routing and damage-entry rolls without selecting a location or motive effect.

The advanced ground table covers every arc, including cross-face hits and its
specific turretless fallback. Armor-gated roll-three motive checks apply the
tracked/wheeled/hover modifiers, steering penalties and speed loss or immobility;
moving vehicles stage the associated wobble broadcasts. Standard/FASA critical-
proof routing sends roll twelve to the hull, even with an intact turret. Advanced
routing takes precedence over critical-proof table selection; immunity still
suppresses its gated motive check and component consequences.

Saved replay preserves the initial routing roll and the additional delegated
FASA/advanced/critical-proof table roll. Tests cover every advanced table row,
turret survival, movement-class motive thresholds, immunity, combined steering
and penetration, FASA motive/turret effects, exact dice order and full rollback.
Target acquisition, complete firing commands, dug-in vehicle state, searchlight
hits, armor warnings and character casualties remain outside this stage.


### Vehicle terrain and optical targeting geometry

Terrain sight lines now sample either Mechs or ground vehicles through one shared
endpoint model. Mobile vehicles use a half-level eye height, reduced to 0.1 while
dug in; stationary units
retain a one-and-a-half-level height. Signed terrain elevation, hover water/ice
support and saved under-bridge position determine the base height. Existing Mech
posture, flight precision, terrain-break height and target-woods behavior remain
in the shared trace. Coordinate sight queries also use this endpoint model.

Explicit visual and light-amplification queries support mixed Mech/vehicle pairs
without acquiring contacts or changing dice. Map light, weather, sensor-disable
bits, sight obstruction and target submersion retain their ordinary effects.
Existing Mech searchlights and nearby infernos can illuminate vehicle targets;
visual and light-amplification queries reflect that lighting immediately and
after restart. Vehicle infrared signatures still return an explicit unsupported
error. Vehicle-owned sensor selection, durable contacts, target locks and full
firing remain unfinished.

Tests cover ridge occlusion at different unit heights, shallow-water separation,
hover surface height, under-bridge replay, mixed optical queries, searchlight
illumination, read-only state and cross-map rejection.


### Persistent vehicle sensor selection

Vehicles now own active sensor pairs and ten-second pending switches, exposed
through the native `sensor` command, Lua selection API and vehicle state. Current
modes remain active until completion; selecting the active pair preserves an
existing request. Daylight rejects new amplification requests and converts active
amplification back to visual. The shared countdown helper discards expired
requests when power is off, the hull is destroyed or required hardware is lost.

Vehicle radar selection requires AntiAircraft technology. Each vehicle probe
occupies one system slot, including Bloodhound; multiple same-family probes stay
available while any matching installation survives. Individual or whole-section
loss returns affected active slots to visual. Pending probe requests recheck
hardware at completion. Snapshots validate equipment and timer bounds. Sensor
selection does not itself enable the still-unimplemented vehicle IR, seismic,
EM, radar and active-probe queries or durable contact acquisition.

The idle server notices pending vehicle sensor work even after shutdown and keeps
the timer within the ordinary save/rollback boundary. Tests cover native/Lua
agreement, script rollback, saved countdowns, daylight changes, redundant probe
loss, unavailable pending hardware, radar admission, invalid snapshots and failed
server-save retries while powered off. Vehicle contacts, target locks and full
attacks remain unfinished.

Vehicle optical acquisition now uses the shared detection actions for Visual and
Light Amplification, against either Mechs or vehicles. Hull direction contributes
100/80/50 for front/side/rear, with 15 added in the original turret construction's
arc. This acquisition bonus follows original turret presence, independently of
weapon readiness. Primary and secondary attempts share the observer's persisted
dice; secondary attempts occur only after primary failure, at half probability.
Invalid paired queries leave the entire stream unchanged. Automatic close-range
and disabled attempts consume no dice. Contact ownership, vehicle scan cadence,
team/hidden facts and target locks remain separate integration work.

Vehicles now own durable contacts through `update_battle_optical_contact` and
`BattleVehicle::contacts`. The shared action acquires unseen targets only when
requested, retains eligible contacts without rerolling, and removes lost contacts.
Visual and Light Amplification contacts can connect Mechs and vehicles in either
direction. Saved contacts validate target existence, distinct identity, a shared
battlefield and at least one observing sensor role. Administrative placement,
removal and object/map purge reconcile observations across both classes. Vehicle
contact displays, automatic scan cadence and target locks remain pending; these
stored observations are last-known state, not a guarantee of current visibility.

Vehicle snapshots now include scenario team, hidden and illuminated flags through
`BattleVehicle::sensor_signature` and the shared trusted signature setter. Scenario
illumination affects optical queries alongside external searchlights. Vehicles
also cache the operator's perception target at startup completion, retaining it
through shutdown, aborted startup and restart until a later startup completes.
Lua `btech.unit.state` returns detached signature and perception values. These
facts prepare automatic vehicle scans; they do not yet enable their cadence,
contact displays or hiding controls.

Automatic tactical scanning now includes vehicles and mixed Mech/vehicle targets
for Visual and Light Amplification. The heartbeat snapshots running observers
before startup advances, visits observers and targets in stable object order,
and commits contact transitions, dice and cockpit messages with the world save.
Failed saves retry the complete update. Character-mode observers remain excluded
under the existing tactical scanner policy. Unsupported mixed sensor pairs are
skipped so they cannot stall other observers; their non-optical query rules still
need implementation.

Vehicle acquisition/loss notices use saved teams, battlefield labels, side arcs
and identification, with shutdown-target notices suppressed. Native and Lua
`brief` support saved vehicle notification preferences and rollback. Vehicle
`AutoconShutdown` and administrator observer controls are not integrated yet.
Vehicle contact lists, detailed scans and target locks also remain unfinished.

Acquired vehicle contacts now appear in native `contacts` and Lua contact queries,
including mixed observer/target classes. Read-only views recheck active sensor
eligibility, terrain, map placement and target lifecycle without acquiring contacts
or consuming dice. Rows use tracked/wheeled/hover/stationary movement labels,
current elevation, side arcs, power/destruction/illumination status and team-based
identity. Saved category filters and vehicle brief modes apply to these rows.
Vehicle default contact displays currently list units only; building contacts,
detailed scans, weapon locks and command-network membership still need vehicle
integration. Unsupported detailed scans and locks return errors before mutation.
Mech targeted-radio reports can address a visible vehicle recipient. The Jeep game
template's multi-token ammunition mode line remains a separate parser coverage gap.

Template syntax now accepts whitespace-separated ammunition flags alongside
pipe-separated flags, including a final dash or numeric brand. Bare known system
entries such as `Ecm` receive empty metadata; abbreviated system data is retained.
Weapon rows still require explicit data/mode fields and validate numeric brands.
The Jeep, HTracked_APC, Huey and Huitzilopochtli assets now parse and construct
without asset edits. The contact-display test uses the real Jeep again.

The ground-vehicle asset audit covers 235 bundled files: 234 decode, 233 resolve
equipment and 232 resolve mass. Remaining diagnostics are J-27_Transport's rotor
section, Shamash's engine weight rating 58, and Svantovit-Streak's ammunition over
capacity. These are distinct remaining construction/asset compatibility issues,
not additional multiword-mode or abbreviated-system parser failures.

The ground-vehicle construction audit now succeeds for all 235 bundled assets.
Hovercraft with an absent engine catalogue rating use the existing one-fifth-tonnage
minimum calculation, matching the reference mass rule; tracked and wheeled
vehicles still require catalogue entries. Shamash retains its diagnostic rating
58 and receives the hover minimum. Two copied assets were corrected:
Svantovit-Streak now records the reference loader's effective 25-round Streak
SRM-4 bin, and J-27_Transport omits a rotor that neither ground-vehicle parser
accepts. The reference tree remains unchanged. The audit example now explicitly
tries `BattleVehicle::new` as well as reporting equipment and mass diagnostics.
Successful construction does not certify tactical simulation or combat parity.

Detailed unit scans and brief unit reports now accept vehicle observers and
vehicle targets. They retain assigned-pilot admission, running scanner checks,
acquired clear visibility and the configured scan radius. Ordinary vehicle scans
show armor/structure condition bands and weapon readiness; administrator-assigned
Mech observers can inspect exact vehicle protection values. Vehicle summary rows
include movement type and turret direction, with heat zero while vehicle heat
simulation remains unimplemented. Computer-derived vehicle ranges are available
in Rust and detached Lua state. Native target labels now resolve mixed classes
without changing the Mech-only membership APIs used by other display paths.

Scan warnings use the recipient's current contacts to identify the scanner and
remain staged until the host transaction commits. Native/Lua unit scans share the
same report, and failed scripts discard warnings. Brief unit reports bypass the
detailed radius but still require scanner hardware and visibility. Vehicle
weapon-lock selection remains unfinished.

Mech coordinate scans and reports also inspect vehicle occupants. Selection walks
all colocated Mechs and vehicles in their shared saved battlefield-slot order,
skipping the scanner and contacts it cannot currently see. The query does not
acquire targets, change locks or consume dice. Unacquired occupants retain the
empty-hex response. Native and Lua coordinate scans use the same warning and
rollback behavior as explicit unit scans. Vehicle observers use the same
coordinate path with their own scanner ranges and saved contacts.

Vehicle observers now support explicit coordinate unit scans/reports and building
or combined building/mine scans. These use shared cockpit admission, scanner
range, terrain visibility and the vehicle's saved perception and random stream.
Hidden structures require an eligible in-character perception attempt; invisible
structures remain undisclosed. Mine scans retain the range gate before perception
and disclose presence only. Native and Lua actions commit dice, experience and
notices together, including rollback after either phase. Visual, light-amplified
and infrared terrain queries share the existing terrain sensor rules. Vehicle
electromagnetic terrain scans remain explicitly unsupported until vehicle signal
and electronic-field support is implemented. Vehicle default target selection is
still pending; explicit coordinates do not establish a weapon lock.

Vehicle contact lists now include structure rows under the same brief and saved
player preference rules as Mech contact lists. Conscious cockpit passengers can
inspect these rows while the vehicle is running; pilot assignment is required for
scan actions, not contact displays. Building rows use current vehicle position,
heading, elevation and terrain sensor eligibility. They disclose integrity and
status according to the silent IdentifyBuilding lock, independently of scan
perception. Hidden buildings require a passing lock, while invisible or unseen
buildings never invoke it. Callback changes are rechecked before emitting a row;
failed callbacks restore world changes and staged notices. Native default/explicit
building lists and Lua building_contacts share this behavior.

Vehicles own the same single unit-or-coordinate target selection shape as Mechs,
with an eight-second settling countdown. A conscious assigned pilot may select an
acquired visible Mech or vehicle, clear the selection, or select valid coordinates
for unit, hex, building, ignition or clearing purposes. Reselection restarts the
countdown. Native lock commands and Lua lock/lock_hex share this state, exposed in
vehicle Lua snapshots. Default scan/report selection now follows vehicle locks;
coordinate selections do not promise that an occupant or building exists.

Vehicle countdowns advance through the ordinary saved server tick, including an
otherwise idle vehicle with no automatic scanner work. Unit lock completion
announces only if currently visible; coordinate locks announce when settled.
Contact loss, administrative placement/removal, shutdown, destruction, completed
sensor changes and forced sensor fallbacks clear the appropriate selection.
Snapshot loading and world validation reject invalid countdowns, power state,
missing/self/other-map targets and invalid coordinates; database repair clears
references to removed targets/maps. Failed scripts and server saves roll back
selection state and notices. Full vehicle firing remains unfinished, and Mech
locks on vehicle targets retain their guard until that attack path supports them.

Conventional aim inspection accepts vehicle shooters and either Mech or vehicle
targets. The shared report combines crew gunnery, range/ammunition rules, vehicle
movement and stabilizer losses, accumulated control damage, target movement,
weapon accuracy, targeting-computer condition, current sensor eligibility and
selected-lock settling. Control damage has its own subtotal field; Mech sensor
critical penalties retain their existing field. Target-side Mech concealment,
homing beacons and friendly TAG assistance feed the existing terms where relevant.

The query clones the shooter's own dice and does not change contacts, ammunition,
recycle state or saved random streams. No acquired eligible sensor or no physical
weapon range means no subtotal. Numeric aim remains separate from firing admission:
a weapon without the selected ammunition can still be inspected. Vehicle heat,
beacon ownership, networks and indirect firing remain unimplemented and do not
contribute live state. Unsupported mixed sensor modes retain explicit errors.

Vehicle friendly-fire safety is stored on the vehicle, defaults off, and is
available to its conscious assigned pilot through `mechprefs FFSafety` and Lua
`unit.friendly_fire_safety`. Vehicle preference listings expose this implemented
setting; Mech preference admission remains unchanged for its other controls.
Lua state and saved vehicle records include the preference.

The Rust `check_battle_vehicle_shot` query validates direct tactical admission and
returns the current aim report without consuming ammunition, recycle or dice. It
checks pilot control, live same-map targets, above-water geometry, mechanical
readiness, weapon arcs, acquired sensors, stealth locking and teammate protection
from both the pilot preference and map policy. AMS and artillery require separate
paths; character attacks remain unsupported. Coolant heat mode permits self-application and bypasses target selection. Out-of-range
conventional attempts retain a missing subtotal, matching the eventual firing
expenditure rule. This check is a prerequisite for a firing transaction, not a
saved authorization or a completed attack; damage and publication remain pending.

The native `weapons` command, shared weapon-status report and Lua `unit.weapons`
now inspect ground vehicles. Stable zero-based indices, vehicle faces, mount flags,
fire/ammunition modes, current supply and mechanical readiness come from owned
vehicle state. Vehicle inspection additionally exposes an optional failure value,
so an electrically disabled or shorted mount does not falsely report destroyed
physical criticals. Cockpit rows distinguish disabled, jammed and shorted states;
Mech wording and its booster summaries retain their existing behavior. One shared
row formatter maintains mode labels and readiness ordering across classes.

These inspections consume no dice, contacts or ammunition. Lua receives detached
rows, and restart reproduces the same listing. Mounted weapons retain their
indices after critical damage. A readable weapon row does not authorize firing.

`launch_battle_vehicle_weapon` resolves a vehicle launch phase on a cloned world
and commits only on success. It accepts the enclosing attack's admitted distance,
computed target number, ECM/Streak-confusion result and glancing policy. It owns
the vehicle's attack dice, ammunition reservation, effective supply fallback and
recycle state. Gatling's supply-limited preparation die precedes attack dice;
failed Streak locks recycle without ammunition loss. Conventional misses and
out-of-range attempts still launch and spend supply. Pod weapons do not glance,
and ordinary Streak locks suppress glancing unless confused.

Normal, heat, gatling, Ultra, rotary, hotload and rapid modes are supported by this launch phase. Ultra
bursts spend two rounds; an attack roll of two permanently destroys the mount
without spending ammunition or recycling. Other mounts remain available. Multi-shot
selections can fall back to normal when supply is insufficient. Rapid and caseless failures resolve internal damage on the shooter using the
request's vehicle critical rules. Caseless attack rolls of two or three consume
a propellant check: seven or less jams the feed; greater than seven destroys
the loader and ignites a misload. Rapid fire destroys its loader on a two and
jams on three or four. Misloads consume only reserved ammunition that survived
the damage cascade, including surviving bins after fatal hull damage. Unsupported
character casualties roll back the whole launch. Rotary and hotload failures save persistent
feed jams without ammunition use or recycling. These jams remain distinct from
temporary critical failures and survive simulation ticks and restart. Physical
mount destruction removes its feed jam. The existing unjam command and Lua action
start a saved 60-second vehicle feed-clearing attempt. Rotary recovery uses
gunnery plus three; other feeds use driving/piloting, with character XP under
the host transaction. Success ejects one round, while empty supply clears without
a roll; stopped, unconscious or destroyed mounts expire quietly. The server
advances idle attempts and rolls back countdowns, dice, ammunition, XP and output
on failed commits. The must-use result
still requires target damage and notices in the enclosing attack transaction;
this phase alone is not the native fire action.

Successful tactical hits on vehicle targets can now use `resolve_vehicle_salvo`.
It shares the existing weapon-group calculation with Mech and terrain attacks,
then applies each packet through vehicle hit routing and critical cascades on one
candidate. Cluster dice belong to the target; partial missile interception rebuilds
surviving packets, and accepted packets continue after fatal hull damage. The report preserves
every located impact and its staged notices. Callers supply the admitted incoming
arc, effective launch modes, guidance context and completed defense count.

This is the grouped target-damage phase, not yet the native vehicle fire action.
Vehicle heat, inferno, beacon and plasma effects remain explicit errors, as do
character casualties requiring the separate character damage path. Ordinary damage,
missile clusters, LBX pellets, bursts, gatling, glancing and armor-piercing routing
use the shared packet rules and existing vehicle impact rules.

`fire_vehicle_shot` now composes tactical admission, committed sensor dice, the
shared launch outcome rules, and existing Mech or vehicle salvo damage in one
transaction. Misses and failed Streak locks preserve target damage state. AMS uses
the shooter's dice and the defending unit's inventory and heat storage.
Mech and vehicle Guardian/Angel emitters and receivers use the same field
calculation, mode controls, transition notices and saved observations. Vehicle
suites require one surviving equipment slot; Mech Angel suites require two.
Shutdown clears both modes and equipment loss disables the affected suite.

The returned shot report retains aim, launch, AMS and a tagged target-salvo report
through the shared native `fire` and Lua `btech.unit.fire` transaction. Both entry
points use the same configured rules, feedback and rollback boundary. Vehicle
beacons, direct thermal effects and inferno outcomes use their dedicated target
handlers. Underwater attacks and character casualties retain explicit guards.


Mech and vehicle firing share launch-failure messages, normal cockpit/observer
messages, Streak/glancing/destruction notices, and Mech salvo consequence collection.
Vehicle adapters supply storage and damage-policy facts; they do not duplicate
launcher rolls, burst fallback, packet construction, AMS rules or unjam behavior.
Third-party interaction audiences include both classes through the existing scanner
view and capture identities before damage. Vehicle unit-at-hex locks resolve their first occupant through the shared slot order;
empty-hex and dedicated terrain actions remain guarded.


Occupied-hex vehicle shots preserve the selected coordinate in their report and use
the common coordinate firing messages. Explicit unit targets take precedence without
changing the lock. Scanning and firing share mixed construction slot ordering:
scanning searches for a visible contact, while firing selects the first non-removing
other occupant and applies normal admission. Hidden or forbidden occupants are not
skipped to attack a later unit. Selection and all shot effects roll back on failure.


Vehicle AMS now uses the same automatic-defense selection, interception rolls,
cluster limits and messages as Mech AMS. The persisted switch defaults off and
is available through `ams`, `btech.unit.ams`, and `btech.unit.state().ams_enabled`.
One ready mount activates per launched missile attack, including misses. Supply
limits rounds spent, while interception remains capped by incoming missiles and
the actual hit cluster. Both laser variants retain their matching-bin requirement.

Construction adapters own only availability, inventory, recycle and Mech heat
storage. Vehicle defense does not add live heat. Physical AMS critical loss disables
the whole capability; temporary mount failures and recycle skip only affected mounts.
Missile firing at vehicle targets no longer rejects installed defenses. Defense
state, shooter dice, ammunition, target packets and output share the host rollback.


Vehicle launchers now use the common immediate-effect resolver for Narc/iNarc
attachment, coolant and heat-mode flamers against either class. Target-owned location dice,
section transfer, ECM updates, pod interception, thermal state and cockpit messages
follow the same rules as Mech launchers. Misses expend normal supply without applying
target effects; host errors roll back state, dice and output. Vehicle `narc`,
`explosive` and `inarc` controls use the existing shared ammunition-mode storage.

Clan plasma follows its existing conventional ten-point damage profile against
both target classes. IS plasma's added heat remains in the Mech impact resolver.
Vehicle targets support attached pods, passive weapon heat, coolant self-application
and their own inferno burning rules. They do not run Mech overheating.


Vehicle targets now retain Narc, homing, haywire and ECM pods on surviving sections.
Pod attachment reuses vehicle hit routing and its direct mechanical effects, without
entering armor/internal damage. Ordinary impacts continue through the same routing
phase into material damage. Mixed target reports retain typed section identities
and one shared pod-message formatter. Native/Lua shots publish location broadcasts
against pre-shot visibility and roll back every consequence together.

Attached effects feed the existing aim, missile grouping and interference rules:
haywire penalizes the carrier, homing assists compatible aim, Narc/homing assists
compatible clusters, and ECM disturbs the carrier. Section destruction removes its
pods; snapshot loading rejects empty effect sets and pods on missing/destroyed
sections. Vehicle live heat remains pending.


`pods` and `btech.unit.pods` now inspect either construction class through shared
row construction and table formatting. Biped swatting remains the single-pod
`removepod` action. Vehicles use `removepods` / `btech.unit.removepods` for a saved
60-second crew action that removes all iNarc effects and preserves ordinary Narc.

The start checks crew authority, power, placement, forward motion, stun and pending
turret/weapon clearing. It rejects duplicate attempts and empty iNarc sets. Speed
commands and firing are blocked while pending. Expiry continues after shutdown;
destroyed vehicles expire silently. The server's ordinary commit covers countdown,
removal and notifications, including retries after failed persistence. The saved
`pod_removal` field is null while idle or contains 1–60 remaining seconds.

Mech and vehicle aim inspection share target movement, TAG assistance, homing-pod
accuracy and concealment calculations. Mechs can inspect aim against a vehicle
without spending dice or ammunition. This query does not authorize a shot:
Mech-to-vehicle tactical locks and conventional firing are integrated as described below.


### Mech-to-vehicle tactical combat

Mechs may acquire vehicle target locks and fire direct conventional weapons at
vehicles, either explicitly, through the saved lock, or through an occupied-hex
selection. Lock countdowns persist and vehicle placement clears stale selections.
Both firing unit classes use the existing vehicle packet, hit-table, critical,
AMS and beacon resolvers. Shooter-owned ammunition, heat, misloads and recoil stay
with the existing Mech launcher. Native and Lua actions publish feedback in the
same transaction as damage, and callback failure restores all affected state.

`BattleShotRules.vehicle_impact` supplies target vehicle policy. The configured
host builds it through the same policy adapter as vehicle firing; the vehicle
wrapper adds only shooter critical policy. `BattleShotReport.salvo` and
`BattleVehicleShotReport.salvo` both use `BattleTargetSalvo`, with `Mech` and
`Vehicle` variants. Lua reads the tag at `salvo.kind` and anatomy-specific fields
under `salvo.report`. Beacon reports use a Mech or vehicle section identity.

This covers tactical conventional damage, Narc/iNarc effects and direct thermal
and inferno effects. Character combat involving vehicles and submerged firing
require their dedicated consequence systems and are rejected before expenditure.
Autopilot and repair work remain outside the active integration scope.


### Empty tactical crews

Tactical crew injury and consciousness continue without an assigned player.
Mechs and vehicles each save a `crew_recovery` component, initialized with its own
private dice when the unit is constructed. It uses the same recovery rules as a
player: injuries during unconsciousness do not restart the timer, and failed
recovery attempts retry after thirty seconds. Fatal injury and material destruction
cancel the attempt. Taking the cockpit transfers pending recovery to that player;
ordinary conscious-control checks then apply. Lua state exposes the remaining
seconds as `crew_recovery_remaining`, never the random stream. Powered-down units
with pending recovery continue to advance through the transactional heartbeat.

IS plasma rifles can fire at ground vehicles from either Mechs or vehicles. They
use ordinary catalogue damage and vehicle hit routing; the additional plasma heat
roll is Mech-specific. Native and Lua firing share this behavior, including
five-point glancing hits and atomic restart/rollback state.


### Vehicle heat and fire

Vehicles save passive `weapon_heat`, including coolant credit. They share firing
heat calculations and direct coolant/flamer effects with Mechs, without Mech
thermal sampling or overheating. Coolant heat mode redirects fire to the carrier.

Inferno selection and missile interception are shared across carriers and targets.
Stationary vehicles accumulate jelly duration. Mobile vehicles use the configured
`fasaadvvhlfire` policy: standard rules roll for an explosion; advanced rules ignite
individual sections. Ignition and minute-long fire pulses use the existing armor,
internal-damage and critical handlers. Repeated inferno hits retain active timers.
`extinguish` and Lua `unit.extinguish` start a two-minute attempt while not running.
Shutdown does not stop fires; destruction cancels them and any extinguishing attempt.

Advanced terrain fire checks occur once per newly entered burning hex. Movement
passes `BattleMovementRules.fall.vehicle_impact`, configured through the same adapter as
firing. The crossing uses shared motive effects or armor damage, and severe results
reuse inferno section ignition. Disabling damage stops travel in the affected hex.
The movement transaction includes the position, damage, timers, dice and notices;
failed publication or unsupported fatal casualties cannot leave a partial crossing.
Lua vehicle state exposes `burning_sections`, `inferno_remaining` and `extinguishing`.


### Vehicle minefield traversal

Vehicles use the shared mine event rules whenever movement enters a new hex.
Mine selection uses current mass and support height. Remote fields do not block
travel, and remaining within the same hex does not repeat an entry check. Mines
resolve before terrain fire; disabling effects stop further crossings. Earlier
crossings still resolve when an unsupported hazard or map edge lies farther ahead.

Scripted mine triggers use the existing `on_mech_mine_trigger` event for vehicles
as well as Mechs. Movement, damage, field removal, random draws and callbacks
share the host transaction. Callback failures or rejected database writes restore
the entire update. Vehicle character casualties and post-destruction heat effects remain
separate unfinished combat work.


### Vehicle follow-up damage

Vehicle salvos and mine blasts finish all accepted packets after a fatal hit,
using the shared vehicle impact and material-damage handlers. Later hits select
locations from the remaining anatomy. A lost face discards the packet after its
diagnostic roll; surviving armor and structure continue to take damage. Mine
blasts also affect existing wrecks in their footprint. Direct new-attack admission
continues to reject destroyed targets. Packet reports, random streams and saved
replay include these follow-up hits.


Vehicle blast heat uses a single response for mines and other admitted blast
sources. Mobile vehicles check fire or explosion even at zero heat, and surviving
wreck sections remain eligible. Stationary units receive six seconds of jelly per
heat point. Mine damage and heat resolve before the entered terrain's fire check,
including when the mine destroys the vehicle; movement then stops at that hex.


Artillery arrivals now damage mixed Mech and ground-vehicle occupants in map-slot
order. Mines and artillery share packet resolution and heat effects, while each
source retains its own footprint and packet sizes. Vehicle hits use vehicle
location tables even for cluster bomblets. Arrival reports contain typed
`BattleBlastImpact` packets and vehicle heat effects; vehicle observers receive
visible arrival notices. Vehicle character casualties remain pending.


Ground vehicles can now fire mounted artillery at a selected hex through native
`fire` or Lua `btech.unit.fire`. They share artillery aim, payloads, queued flights
and feedback with Mechs, while the common vehicle launch handler applies their
ammunition, recycle, heat and hotload failures. Automatic observation by a friendly
Mech or vehicle now contributes
saved trajectory correction, applied to subsequent launches and cleared on retargeting.
The observer is chosen in map-slot order before its running-state check.


Explicit spotter links now support Mechs and ground vehicles in either role.
Use `spot #your-unit` to declare spotting, `spot #observer` to select a friendly
acquired observer, or `spot -` to clear the role. Spotting prohibits firing; using
an observer limits fire to indirect-capable weapons and artillery. Missiles use
the observer's acquired unit target, and artillery uses its visible selected hex.
Observer movement, skill, target settling and optics use shared aiming rules.
Links persist across restart and are revalidated when used. Retargeting an observer
clears dependent artillery corrections on both Mechs and vehicles.


Ground vehicle state exposes `c3_hardware` through Lua, including installed and
working master counts and slave/C3i availability. These are derived from current
equipment using the same inventory rules as Mechs. Each vehicle master and C3i
computer occupies one slot. A destroyed master installation disables classic C3
if no complete master survives, even when a slave remains intact. Vehicles participate in C3 and C3i networks through the same commands, Lua APIs,
capacity rules, messages, status/contact reports and range sharing as Mechs.
Shutdown and ECM temporarily suppress data exchange; changes of team or map and
loss of working computers clear the affected links.


`btech.unit.aim_hex` also accepts ground vehicles. It previews the selected terrain
mode using current weapon, movement, control damage and C3/C3i range rules, without
consuming dice or ammunition. Target movement, weapon-lock settling and optical
unit-target terms do not enter a coordinate preview. Vehicle coordinate fire uses the shared hex-shot pipeline for empty coordinates,
woodland ignition/clearing, building damage and surface fracture. Launch failures
use a tagged Mech/vehicle damage report. Vehicle character-combat remains unfinished.


Vehicle Lua state includes `flooded`, a permanent water-destruction flag distinct
from hull damage and crew injuries. A flooded vehicle is a wreck even when its
armor remains intact. The environmental state operation stops its controls and
active fire/recovery events while preserving material and ammunition. Ice and bridge failures dispatch mixed occupants in map order, with the breaker
last. Vehicle falls share damage and direction calculations with Mechs, then use
vehicle pilot checks and packet impacts. Surviving affected vehicles flood after
the fall; hovercraft are excluded from ice falls. Bridge occupants outside the
fall selection retain their height until they move. Terrain, damage, dice and
flooding publish together and survive restart.


Vehicle `mechprefs AutoFall` and `btech.unit.auto_fall` use the same cockpit
permissions, toggle/explicit settings and feedback as Mechs. The saved preference
is exposed by `btech.unit.state`. Enabled auto-fall skips downhill cliff avoidance
only when a pilot is assigned; uphill checks and pilotless automatic stops remain.
Fall injury and damage still consume their ordinary dice. Tests verify exact dice
ordering, native/Lua controls, rejected-callback rollback and SQLite replay.


Ground vehicles can cross occupied hexes. Ordinary stacking collision rules apply
to Mechs, and do not count ground vehicles or trigger for vehicle movers. Crowded
travel therefore retains the same motion and dice as travel through an empty hex.


### Ground vehicle water entry

Tracked and wheeled vehicles now resolve ordinary water entry using a speed-based
pilot check. Successful avoidance restores the previous position and stops;
failure floods the vehicle without fall damage or crew injury. Hovercraft,
waterproof vehicles and zero-depth water bypass the check. Cliff and reverse-slope
handling take precedence, and water avoidance skips ordinary step mines and fire.
Pilotless units stop without dice. Character flooding remains guarded until its
casualty effects are implemented; rejected entry restores the complete movement
transaction. Tests cover both drivetrains, smoke overlays, exact dice use, unchanged
material/ammunition/crew, successful stops, flooding, exemptions and SQLite replay.


Hovercraft low-span collisions share pilotless exemptions, control rolls and
experience handling with water and reverse-slope checks. Cliff checks use the same
helper with experience disabled. Bridge collision tests cover successful stops,
failed impacts under ordinary and skid rules, pilotless stops, exact dice sequences,
restored support height and SQLite replay.


### Vehicle trees and rocks

With `newterrain` enabled, tracked vehicles check heavy forest; wheeled vehicles
check either forest type and rough ground. Hovercraft check either forest type
regardless of that setting. Checks apply above one movement point of speed and
use the shared terrain-control path, including the pilotless exemption and XP.
Heavy forest adds difficulty for wheeled vehicles and hovercraft. Failure causes
a speed-based fall, then ordinary slope, mine and fire entry effects continue.
The obstacle check precedes slope speed loss. Tests cover all movement types,
configuration exemptions, smoke-covered terrain, successful and failed checks,
exact dice use, character-failure rollback and SQLite replay.


### Vehicle ice travel and falls

Tracked and wheeled vehicles roll the shared one-in-six ice check on entry at
surface level. Hovercraft and submerged vehicles do not consume that roll. Intact
ice supports surface travel at water level; a waterproof vehicle already below ice
retains the waterbed height. A fracture changes terrain before dropping neighbors,
then the triggering vehicle. Waterproof equipment protects only the trigger from
post-fracture water destruction; neighboring vehicles still flood. Zero-depth ice
can break without falls or flooding. Vehicle falls onto ice use the same fracture
handler before applying their remaining damage, preserving nested water falls.

Tests cover both drivetrains, hover and submerged exemptions, smoke overlays,
ordered mixed occupants, trigger/neighbor waterproof differences, zero-depth ice,
exact dice, nested falls, rollback and SQLite replay.


### Shared character pilot injuries for vehicles

The character pilot injury action accepts Mechs and ground vehicles through the
same health, consciousness and recovery implementation. Vehicle character status
is saved independently of the six-hit tactical injury counter and exposed in Lua
unit state. Fatal character injury releases the pilot and stops vehicle controls
without inventing armor damage, ammunition loss or flooding. The shared evacuation
action moves non-wizard occupants and retains experience using the configured
rules; failure restores both unit state and occupant locations.

Tests cover zero-hit behavior, injuries beyond the tactical limit, unconscious
control rejection, fatal cleanup, wizard exemption, failed evacuation rollback,
invalid active wreck snapshots and SQLite/Lua state. Combat and environmental
vehicle casualty chains remain guarded pending integration; this adds the shared
injury and evacuation foundation rather than enabling those callers.


### Character injuries from vehicle criticals

Mech impact injuries and vehicle crew-hit/Gauss-explosion injuries now use one
health dispatcher. Assigned in-character pilots use character health and recovery;
virtual crews use tactical health. Critical reports retain direct character injury
results separately from tactical injuries, with nested explosions retaining their
own ordered results. The vehicle critical action publishes notices, visible
broadcasts, character feedback and newly fatal evacuation in one rollback checkpoint.

Tests compare crew-critical health and random streams with the shared standalone
injury action, cover fatal evacuation rollback, and verify surviving Gauss-explosion
injuries after internal damage and SQLite reload. Instant crew-kill criticals,
fatal hull damage and vehicle catastrophes still require their distinct character
casualty rules; direct character vehicle firing remains guarded.


### Instant vehicle crew death

Crew-killed criticals now record a distinct persistent crew-loss cause, exposed as
`crew_killed` in Lua state. They stop vehicle controls and release the pilot without
changing tactical injury counts, character health, armor or ammunition. All three
critical tables use the shared evacuation action, including vehicles without an
assigned pilot. Nested weapon explosions finish their material damage after crew
loss and publish evacuation through the same action checkpoint.

Tests cover each table, assigned and virtual crews, wizard exemptions, invalid
active-wreck snapshots, failed evacuation rollback, SQLite/Lua state and nested
Gauss explosions. A hotloaded explosion that also destroys the hull still rolls
back: fatal hull and catastrophe casualty rules remain unfinished, as do direct
character vehicle firing and VTOL integration.


### Vehicle hull loss and catastrophe occupants

Ordinary hull destruction leaves occupants alive inside the wreck. FASA power-plant
CASE containment destroys the rear section without killing the crew; fuel and
uncontained power-plant explosions mark crew death and destroy all remaining
sections. Nested critical explosions and remaining internal damage finish within
the same candidate. Character health is not rewritten to represent these deaths.

Located armor-damage and critical actions share notification, character feedback,
casualty publication and rollback. Tests cover ordinary hull survivors,
through-armor crew criticals, containment, fuel explosions, nested weapon/ammunition
cascades, failed evacuation, wizard exemptions and SQLite replay. Direct character
vehicle firing, environmental caller integration, transported-unit destruction and
VTOL support still need work. Excluded unit classes retain their admission guards.

Ordinary hull loss from terrain or scheduled section fires likewise preserves
occupants. Fire damage still rejects changes to character injury or crew-death
state because its publication path is unfinished; a seeded crew-kill fire test
verifies complete rollback, while hull-only fire tests verify surviving occupants.


### Vehicle fire character publication

Scheduled fire ticks retain both occupant notices and ordered character injury
reports. Their server action publishes injuries and newly lethal evacuation in the
same rollback checkpoint as armor damage. Terrain-fire entry retains these reports
through the existing movement action. Shared vehicle injury traversal preserves
nested critical order without repeating health or evacuation logic. Raw movement
continues to reject character crew consequences that require the host action.

Tests cover nonfatal/fatal scheduled injury, crew-kill exposure, terrain-fire
movement evacuation, failed afterlife rollback and persisted replay. Existing fire
save-failure tests exercise the server transaction. Inferno/heat attack admission,
other character combat callers, transported-unit destruction and VTOLs remain
unfinished; autopilot, repairs and excluded unit classes remain out of scope.


### Character vehicle inferno and blast heat

Inferno and heat exposure actions now share vehicle damage publication, ordered
character injury feedback and fatal evacuation. Standard mobile heat preserves
its explosion check, including zero heat; advanced fire retains section damage
and later fire timers. Mine and artillery actions publish both material-packet
injuries and subsequent heat injuries through one shared blast visitor. Raw blast
APIs still require non-character victims because they do not publish casualties.

Tests cover the explosion threshold, zero-heat explosions, advanced-fire feedback
exactly once, fatal evacuation rollback, mine-field rollback, artillery cursor
rollback and persisted replay. Direct character weapon firing, remaining movement
casualties, transported-unit destruction and VTOL integration are still unfinished.


### Shared character direct firing

Native and Lua firing now admit character Mechs and ground vehicles through the
same configured action. Both target types retain ordered injury reports for shared
feedback and casualty publication. Direct, coordinate and artillery launcher
misloads use the same injury traversal as other vehicle internal damage. Launcher,
ammunition and defense calculations remain shared across unit types.

A four-way attacker/target test covers Mech and vehicle combinations, native/Lua
state equivalence, fatal pilot evacuation, missing-afterlife rollback and SQLite
replay. The standalone tactical shot query and resolver retain their character
admission guard; the configured host action owns casualty publication.

Character automatic contact acquisition and vehicle gunnery experience remain
unfinished, alongside movement casualties, transported-unit destruction and VTOL
integration. These direct-fire tests acquire contacts before enabling character
flags; they do not establish complete character combat gameplay.


### Automatic character contact acquisition

Character Mechs and ground vehicles participate in the shared automatic optical
scanner. The scanner uses each unit's cached startup perception, active sensors,
visibility and saved dice, with the same contact retention and loss rules. Startup
and unsupported mixed sensor pairs retain their existing eligibility rules.

Tests compare character and tactical allied formations, replay contact loss after
SQLite reload, and verify that a failed server save rolls back acquisition for
both character unit types. The direct-fire matrix now acquires contacts with
character flags already enabled. Hostile character perception experience awards
remain unfinished; enabling acquisition does not complete the experience system.


### Shared automatic perception experience

New hostile contacts between character units draw a one-in-six experience gate
from the observer's saved dice after acquisition succeeds. Mechs and vehicles
then share the existing perception check used by building and mine scans, with a
minus-two modifier and the normal skill-award interval. Inactive pilots do not
spend skill-check dice; allies, non-character pairs and retained contacts do not
attempt acquisition experience.

Contact events retain accepted XP diagnostics for server publication in the same
transaction as contacts, dice and character skill changes. Tests cover both
chassis types, gate failure, failed skill checks, disconnected pilots, rate-limited
awards, exact dice consumption, retained contacts and SQLite replay.


### Shared classic gunnery experience inputs

The standalone classic award API accepts Mech and ground-vehicle attackers and
targets through shared eligibility and formula inputs. Vehicle unit XP settings
persist with construction state and use the same validation and administrative
setter as Mechs. Classic difficulty uses available maximum speed after motive
damage for vehicles and existing effective-speed rules for Mechs. Its award die
uses the shared unit dice accessor; character award calculation is unchanged.

Tests exercise all four participant combinations, per-unit scaling, classic
suppression semantics, continuous skill awards, invalid settings, eligibility rejection
without state changes and SQLite replay. Vehicle battle-value valuation and
configured firing's per-packet XP integration remain unfinished. Battle-value
eligibility continues to exclude vehicles until that valuation is implemented.


### Ground-vehicle battle value and experience

Ground vehicles expose current battle value using shared weapon heat ordering,
defensive equipment values, movement bands and decimal rounding. Vehicle inputs
supply live hull protection, available motive speed, installed cooling, movement
discounts and Gauss exposure. Installed weapons and ammunition retain valuation
after expenditure; vehicles do not receive Mech gyro bonuses or explosive-bin
penalties.

The battle-value experience formula now accepts Mech/vehicle combinations through
shared eligibility, target suppression, optional pilot modifiers and skill awards.
It consumes no award dice. Tests check a hand-calculated Demolisher, live armor
loss, all four ground movement discounts, finite values across the shipped ground
fleet, mixed participant awards, suppression and SQLite replay.

Configured vehicle firing still needs per-packet XP contexts and publication.
VTOL valuation and integration remain pending with the VTOL construction model.


### Shared per-packet firing experience

Configured native and Lua firing now pass the same XP context to Mech and vehicle
target salvos, regardless of shooter type. Vehicle packet reports retain each
pre-impact award and its diagnostics. Eligibility is rechecked before each packet,
so a destroyed target stops earning XP while vehicle damage packets retain their
existing completion behavior. Inferno and dedicated non-material effects keep
their separate resolution paths.

Target consequence publication owns gunnery diagnostics for both report types,
avoiding duplicate publication when a Mech fires. Vehicles use Gunnery-Conventional
when extended gunnery is disabled; both formulas share this skill selection.
Tests cover classic and battle-value XP across all four shooter/target pairings,
native/Lua equivalence, fatal-casualty rollback, multiple missile-packet awards,
callback rollback and persisted replay. VTOLs and remaining movement/transport
casualties remain unfinished.


### Character-capable vehicle fall action

The vehicle fall host action applies personal injury through the shared pilot
health dispatcher and protection XP through the shared control-check award code.
Its report retains character injury separately from tactical crew injury, with
ordered XP diagnostics. The action publishes personal injury, nested reported
fractures, material critical injuries and mines before shared casualty evacuation,
restoring world state and buffered effects if publication fails.

Tests cover zero-severity personal injury, fatal evacuation and rollback, exact
replay, SQLite persistence and successful protection XP with material fall damage.
The raw vehicle fall API remains tactical. Movement and surface callers still need
to adopt the character-capable path; water/flooding and VTOL integration remain
unfinished. This action does not remove those existing admission guards.


### Character vehicle collision falls during movement

Movement now retains vehicle fall reports alongside Mech fall reports. Low-bridge
collisions, slope/cliff failures and tree/rock impacts use the shared character-
capable fall resolver when called through the movement host action. The existing
movement transaction publishes fall XP, personal injury, packet critical injuries
and fall-triggered mines before shared casualty evacuation. Ordinary notices are
still staged once by the movement result.

Tests drive character vehicles into forest, cliff and low-bridge hazards, covering
nonfatal injury, fatal evacuation, failed-afterlife rollback, raw tactical guards
and SQLite replay. Surface-fracture callers and water/flooding casualty handling
still require integration; VTOLs and transport consequences remain unfinished.

### Character vehicle falls through broken surfaces

Surface fracture uses the same character-aware vehicle fall resolver as direct
falls and movement collisions. One host publisher traverses Mech and vehicle
fall reports for ice/bridge actions, nested falls and weapon surface impacts.
Ground movement and jump landings retain vehicle reports from affected neighbors
alongside their Mech reports, including interrupted jumps and upward ice breaks.
This keeps character feedback and fatal evacuation in the enclosing transaction.

Tests cover survivable and fatal character injuries on ice and bridges, missing
afterlife rollback, deterministic replay and SQLite persistence. The movement
collision matrix also covers ice entry with a fixed fracture roll. Flooding a
surviving hull leaves ordinary occupants aboard; a fatal pilot injury follows the
shared casualty path. Transported-unit destruction and the remaining vehicle
water-entry admission are separate unfinished work, as is VTOL integration.

### Character vehicle water entry

Ordinary failed water avoidance and downhill cliff entry now use the enclosing
host movement transaction for character vehicles. They share the existing
flooding state transition, terrain control checks and fall consequence publisher;
no additional crew-injury model is introduced. Flooding alone disables the hull
without relocating or injuring ordinary occupants. Preceding fall injuries still
use the shared character casualty and rollback path.

Tests cover tracked and wheeled avoidance success/failure, unchanged occupant
health and material armor, raw-action rejection rollback, persisted replay, and
survivable/fatal cliff falls into water. Transported-unit destruction, towing and
VTOL integration remain unfinished.

### Shared transport destruction

The host casualty boundary now propagates newly destroyed carriers through their
in-character unit contents when `transported_unit_death` is enabled. Mechs and
vehicles share one queue for nested carrier losses, ordinary teleport callbacks,
placement and notification. A dedicated saved transport-loss flag disables each
cargo hull without fabricating armor damage, cockpit hits, flooding or crew death.
Disembarkation failure restores the enclosing combat action and its effects.

Tests cover both nested chassis orders, the configuration switch, preserved crew
health and armor, failed destination rollback, deterministic replay and SQLite
persistence. Loading/cargo commands, towing, detailed disembarkation movement
rules and VTOL integration remain unfinished.

Already-destroyed in-character cargo is also disembarked on carrier loss. Its
existing wreck state is preserved, and that move does not start another
transport-destruction cascade through its own contents. The nested transport
matrix covers this distinction for both chassis types, with the configuration
switch, rollback and persisted replay.

### Shared VTOL asset anatomy

VTOL assets now use `BattleVehicleTemplate`, hull sections, weapon mounts and
ammunition bins shared with ground vehicles. Rotor anatomy has its own section
identity. Class remains separate from movement: both bundled observation VTOLs
retain their stationary movement. Rotorcraft suspension uses the 50/95/140 rating
bands and shared vehicle engine arithmetic; rotor components use the existing
rounded ten-percent component mass calculation. Piloting skill selection for VTOL
movement uses Piloting-Aerospace.

All 34 bundled VTOL assets parse and resolve their equipment without a separate
launcher or ammunition implementation. Tests cover serialized template replay,
invalid mixed anatomy, suspension boundaries, component mass and rejection of
forged aircraft movement in ground construction. Live VTOL construction remains
explicitly gated until flight movement and rotor consequences are implemented.
Ground critical tables and rotor firing geometry do not stand in for those rules.

### VTOL hit and rotor critical selection

VTOL definitions now select standard/FASA hit locations from supplied rolls using
shared `BattleVehicleHit` and hull-face identities. Rotor damage, rotor destruction
and FASA side-nine main-weapon destruction remain explicit secondary outcomes.
Critical-proof equipment selects standard locations and suppresses those effects.
Advanced rotor critical rolls distinguish main-rotor damage, tail-rotor loss and
rotor destruction. Selection does not draw dice or mutate material; the future
flight impact transaction must apply the reported consequences together.

Tests exhaust every arc, 2d6 result and critical-proof/FASA combination, along with
advanced rotor critical boundaries and invalid input. These selectors do not yet
admit live aircraft: rotor lifecycle, crash handling and flight motion remain
unfinished, and the live-construction gate remains in place.

### Shared rotor material state

`BattleVehicle` can now construct rotorcraft material independently of world
admission. Main-rotor hits use shared motive speed loss and section destruction;
rotor section loss removes lift and halts horizontal motion without destroying
the hull or killing crew. Repeated hits report whether lift was newly lost so a
future flight action can schedule a crash once. Tail-rotor loss is saved separately;
flight control limits and crash scheduling remain the host flight action's work.

Tests cover all eighteen one-MP losses on a Kestrel, fallen-unit minor hits,
repeated tail/main rotor hits, direct section destruction, unchanged hull/crew,
snapshot replay and corrupt horizontal motion on a rotorless aircraft. Construction
and snapshot loading share anatomy validation. World creation and world validation
still reject VTOL admission until the flight lifecycle is implemented.

### VTOL velocity budget and tail-rotor controls

Horizontal and vertical speed limits now share a single perpendicular-velocity
calculation. Requests account for current rotor/motive damage; an overspeed
component leaves no capacity on the other axis. Horizontal requests share forward,
reverse and cruise limits, including the tail-rotor cruise tolerance. A separate
transactional control step constrains an existing forward command after damage
without erasing actual momentum or changing a reverse command.

Tests cover both axis signs, the 3-4-5 velocity relation, damage and lift loss,
forward/reverse limits, tail-rotor rejection, repeated clamping, nonfinite-input
rollback and saved material replay. These methods are flight-control primitives;
authority, fuel, takeoff, live movement and crash scheduling remain unfinished.

### Saved VTOL fuel consumption

Rotorcraft material now owns fuel capacity and remaining inventory. Omitted fuel
uses 4000 units; saved fuel must match the authored capacity and cannot fall below
the exhausted sentinel. Movement-event consumption uses the vehicle's shared dice
stream, low-speed half-cost chance, altitude-dependent overspeed cost and optional
fusion-fuel exemption. Reaching zero and announcing exhaustion are distinct checks.
New exhaustion halts horizontal motion without damaging rotors, hull or crew;
airborne aircraft now enter the saved falling phase. The future flight event must
publish the outcome and integrate the descent.

Tests cover deterministic dice replay, skips without draws, capacity validation,
zero/exhausted transitions, repeated checks, the altitude boundary and preserved
material. Takeoff, live flight event wiring and crash scheduling remain unfinished.


### Saved VTOL takeoff and lift loss

Aircraft own one flight phase: landed, launching with a countdown, airborne, or
falling. A zero-delay request lifts off on the next event; delayed requests and
cancellation survive serialization. Liftoff rechecks placement, running power,
rotor capability, ceiling and fuel before stopping shared horizontal motion and
setting vertical speed to 60. Rejected requests leave material unchanged.

Rotor section loss, hull destruction and fuel exhaustion use their existing shared
vehicle transitions to cancel a pending launch or begin one fall. Repeated loss
preserves an ongoing descent. Tests cover countdown replay, cancellation, request
and event checks, fuel exemptions, shared damage, and invalid saved timers.

These are material transitions. Live VTOL admission remains gated pending flight
event integration, altitude movement, landing and crash handling. The host must
also enforce cockpit authority, launch-delay permissions, fortified state and
visibility changes before exposing takeoff commands.

### Controlled VTOL descent and landing

Airborne rotorcraft can set vertical speed within the existing shared velocity
budget. Admission checks power, lift and fuel, and rejected requests preserve
state. Deliberate landing checks height, actual reverse speed, commanded/vertical
speed difference, vertical limits, and grass/road/building terrain. Touchdown stops
actual motion, preserves the horizontal command, clears flight state and records
the surface height. Landing during a queued launch cancels it.

Tests cover takeoff-to-descent-to-landing through public material methods, saved
replay, exact speed and height boundaries, terrain rejection, fuel exemption and
lost lift. Landing returns an explicit touchdown result for future host altitude,
messages, callbacks and mine activation. Live VTOL admission remains gated until
movement, landing effects and crashes are integrated.

### VTOL movement projection and surface decisions

Flight movement proposals reuse vehicle horizontal projection, including reverse
travel and map movement modifiers. Vertical movement retains fractional elevation
levels independently of the horizontal modifier, and the orbit ceiling returns
altitude 299 with vertical motion stopped. Surface decisions distinguish underwater
entry, bridge underside contact, and ordinary ground contact. Ground contact
reports the fall severity for the host to use if deliberate landing fails.

Tests cover fractional movement, reverse geometry, horizontal modifiers, ceiling
and invalid inputs, saved replay, water truncation and bridge clearance. These are
proposals: the host must retain continuous altitude, trace all crossed hexes, apply
landing or crash effects, and commit state. Live flight admission remains gated.

### Durable VTOL altitude and movement commits

Continuous altitude now belongs to saved aircraft flight state. Placement starts
at the map surface; takeoff, launch cancellation and lift loss preserve height.
Landing and movement read that saved altitude, and touchdown records the surface
height there. Shared elevation queries use aircraft altitude for rotorcraft.
Snapshots reject nonfinite or out-of-range heights.

Movement proposals retain their origin and movement modifier. A commit recomputes
the proposal against current state and rejects stale or modified values before
changing the existing vehicle position and motion. Fractional height and ceiling
velocity changes commit together. Tests cover save/load between successive events,
height after lift loss, elevated launch cancellation, and rejected proposal replay.

The host still must trace terrain and resolve landing/crash consequences before
committing a proposal. Live VTOL admission and event scheduling remain unfinished.

### VTOL terrain traversal and clear-path advancement

Aircraft movement now checks ordered hex intervals using the same boundary
partition as ordinary movement. Each interval also checks altitude bands, so an
intermediate hill or a bridge span cannot be skipped merely because the movement
endpoints are clear. The first obstructed hex reports a representative contact
point and height; water and map edges remain distinct outcomes.

Clear paths commit vehicle coordinates, motion and continuous altitude together.
Obstructed paths leave material and dice unchanged for the enclosing host action
to resolve. Tests cover an intermediate hill, an entire bridge-band crossing,
under-bridge clearance, water entry, map edges and saved replay of successful
movement. Live scheduling and collision/landing effects remain unfinished.

### VTOL automatic landing and water destruction

Airborne environmental advancement now resolves clear movement and the first
surface contact on a candidate copy. Ground contact calls the ordinary VTOL
landing method; successful touchdown commits surface altitude and position.
Water contact uses shared vehicle flooding, preserving armor and crew injury
state while shutting down the wreck. Boundary contacts are nudged into the
reported hex so continuous and discrete positions agree after saving.

If deliberate landing fails, the result reports the required fall severity and
contact while leaving state unchanged. Host VTOL hit routing and the shared fall
transaction must resolve that crash before flight admission can open. Landing
mines, callbacks, notifications and casualty publication also remain host work.
Tests cover touchdown replay, intermediate-hex landing, flooding persistence and
atomic crash deferral.

### Shared advanced rotor critical resolution

The existing vehicle critical transaction now routes advanced rotor criticals to
rotor damage, tail-rotor loss or main-rotor destruction. Selection uses the same
saved dice and suppression rules; application reuses rotor material changes and
tail-rotor cruise limits. Rotor destruction enters the saved falling phase without
changing altitude or inventing hull and crew damage.

Tests cover every 2d6 outcome, airborne lift loss, replay, saved material, disabled
criticals, combat safety, critical-proof equipment and rejection after rotor loss.
Ground critical tests continue to reject rotor outcomes. Other VTOL hit/critical
routing, crash damage publication and live flight admission remain unfinished.

### Advanced VTOL hull critical routing

Advanced rotorcraft front, side and rear critical rows now select aircraft
outcomes, including stationary observation craft. Copilot and pilot effects reuse
shared firing and piloting penalties with aircraft feedback; they do not invent
personal injuries or crew stun. Weapon, ammunition, stabilizer, sensor and crew
outcomes retain the shared critical application paths.

Tests enumerate all hull rows and rolls for mobile and stationary VTOLs, resolve
both cockpit-control outcomes, and verify saved replay. Airborne engine hits
explicitly reject before committing dice or damage until emergency landing and
crash resolution are wired. Standard VTOL critical routing, ordinary hit routing
and live flight admission remain unfinished.

### Standard VTOL criticals and catastrophic surface placement

Non-advanced VTOL criticals now use their own single-d6 row for mobile and
stationary aircraft. The ground FASA setting does not change that row. Cockpit
loss, main-weapon jams, engines and explosions reuse shared critical handlers;
the ground preliminary motive roll is not drawn. Aircraft power-plant containment
uses installed CASE independently of the ground FASA rule.

Aircraft catastrophes with battlefield placement first resolve the local surface
and settle altitude and vertical speed before shared destruction. Missing map data
rejects the entire critical without consuming dice. Tests cover all standard
outcomes, ignored ground-table settings, replay, explosion height, CASE containment
and missing-map rollback. Airborne engine emergency landing/crash resolution,
ordinary VTOL hit routing and live flight admission remain unfinished.

### Standard and FASA VTOL impact resolution

Ordinary rotorcraft location selection now enters the shared vehicle armor and
critical pipeline. Standard, FASA and critical-proof routing retain their draw
order and apply rotor side effects before armor. FASA main-weapon destruction
reuses the same intact-mount ranking as main-weapon jams, then shared equipment
loss handling. Rotor hit feedback is shared with rotor critical feedback.

Tests enumerate every location roll and arc across those policies, including
rotor damage/destruction, main-weapon loss, armor, replay, combat safety and atomic
rejection. Existing ground impact and jam tests exercise the extracted helper.
Advanced VTOL hit routing remains an explicit error until its aircraft table and
armor gate are wired. Emergency engine landing, crashes and live admission also
remain unfinished.

### Advanced VTOL hit locations and armor gating

Advanced aircraft impacts now route through their own hull/rotor table, including
surviving-turret fallback, then use the shared armor critical eligibility check.
The check preserves its dice draws even on table entries without a critical flag.
Critical-proof equipment suppresses that check without replacing advanced hit
locations. Advanced hits leave rotor effects to armor and critical resolution.

Tests enumerate all arcs and rolls, lucky full-armor checks, damaged armor,
critical-mode suppression, invalid-input dice preservation, turret destruction,
and shared impact replay. The prior advanced-hit rejection is removed. Host
configuration selection, emergency engine landing, crash publication and live
flight admission remain unfinished.

### Per-target host critical configuration

Configured vehicle damage policies now retain separate ground and aircraft table
choices. Shared hit and critical boundaries select from the victim's class, so
`fasaadvvtolcrit` controls aircraft independently of `fasaadvvhlcrit`; FASA remains
the fallback. Explicit material rules can omit the aircraft override and apply one
table to both classes. Nested launcher policy updates preserve both choices.

Tests cover all switch combinations, stationary observation aircraft, direct hit
and critical dispatch with opposing ground/aircraft choices, and launcher-policy
inheritance. Existing host callers obtain both choices from the same configured
policy constructor. Emergency landing, crash resolution and live aircraft admission
remain unfinished.

### Successful engine-loss emergency landings

Airborne engine criticals can now complete a successful emergency landing on a
supported surface. They use the shared vehicle pilot check with height above the
surface as modifier and retain the check in the critical report. The configured
extended-piloting setting travels with the shared critical policy. Success disables
propulsion and commits landed flight state at the surface without armor or crew
damage. Deliberate and emergency landing share surface eligibility.

Tests cover standard and advanced engine criticals, pilot targets, success replay,
saved material and failed-check rollback. Failed checks and unsuitable terrain
still require crash resolution and leave the public world unchanged. Character XP
publication, landing effects, crashes and live flight admission remain unfinished.

### Saved forced VTOL descent

Failed engine-loss landing checks and unsuitable terrain now commit disabled
propulsion and a forced-fall cursor instead of rejecting the critical. Supported
terrain retains the failed pilot check; unsuitable terrain draws no landing roll.
Rotor, fuel and hull lift loss also start the same shared free-fall cursor. Repeated
loss preserves descent timing and altitude.

The cursor reuses BattleMech forced-descent cadence and impact-severity arithmetic.
Aircraft retain fractional starting altitude until the first descent event, then
save the integer descent height. Impact outcomes leave state unchanged until the
host can commit crash damage. Saved aircraft require matching flight phase, cursor
and altitude. Catastrophic explosions settle and clear the cursor at the surface.

Tests cover shared-clock replay, saved countdowns, impact retries, corrupt cursors,
failed emergency checks and water terrain without pilot draws. Crash damage,
powered recovery, landing effects, character XP and live scheduling remain
unfinished; this is forced descent, not a claim that all flight recovery rules
are complete.

### VTOL crash material through shared vehicle falls

Aircraft crash material now settles the aircraft and clears its descent cursor
before using the existing vehicle fall resolver. Pilot checks, injury, damage
arithmetic, five-point packets, hit tables and mine activation have one
implementation. The composable resolver accepts wider severity values for forced
falls; ordinary ground fall callers retain their existing API and final world
validation. Non-safe crashes prevent relaunch through shared motive immobilization.

Tests cover airborne and falling aircraft, zero and high severity, water damage
reduction, saved replay, combat safety and transactional rejection. A nested mine
admission failure rolls back earlier crash damage and dice. Ice and mine effects
retain their admission checks rather than bypassing live-world validation.

This is a material component: contact placement, flooding, destroyed-hull descent,
powered recovery, character consequences, publication and live flight scheduling
still require host integration. Aircraft creation and live-world admission remain
gated. Rotor crash feedback and relaunch prevention do not fabricate rotor-section
armor damage; ordinary impact packets still determine section damage.

### Atomic forced-descent events

The VTOL world component now advances the saved shared fall cursor and resolves
its impact in one candidate transaction. Waiting and descending preserve damage
and dice; impact returns the existing vehicle fall report and clears the cursor.
A rejected impact leaves the due event unchanged for retry. Restart tests serialize
and reload between ticks and compare the results against the shared descent clock.

Mechs and VTOLs now also share supporting-surface selection. An aircraft below a
bridge falls beneath the deck instead of being moved up onto it by crash settlement.
Tests cover deck boundaries, ice, water, descent timing and impact rollback.

This event still requires the live host flight dispatcher and publication boundary.
Character aircraft and destroyed-hull descent remain explicit unresolved cases;
powered recovery and immersion are not implemented by the forced-descent component.

### Forced descent in movement dispatch

The shared movement transaction now advances falling VTOLs in stable unit order,
including powered-off aircraft with no horizontal motion. Crash notices and vehicle
fall reports flow into the existing host movement publisher, which owns character
injury feedback, mine callbacks and casualty evacuation. The host descent path
passes character admission through to the shared fall resolver; the tactical API
continues to reject character aircraft. Ground driving excludes aircraft, and a
pending forced descent keeps the server's one-second update active.

Tests compare dispatched descent with the individual fall event, verify exactly-once
impact feedback and prevent aircraft from taking a ground-motion step. A host test
reaches final aircraft admission and verifies rollback of the due cursor, material
and staged messages. This does not prove live aircraft admission: the gate remains,
as do nested character and terrain validation checks. Takeoff and ordinary flight
dispatch, immersion, wreck descent and powered recovery remain unfinished.

### World-backed flight contact resolution

The world flight-contact component now resolves the existing path and landing
rules against current base terrain. Asset tests and world movement share a tile
lookup interface and contact placement; no map copy or second terrain classifier
is needed. Ground contact that cannot land returns a completed shared vehicle fall
report, committed together with contact position. Rejected consequences restore
pre-contact position, height, material and dice.

Direct water contact uses shared vehicle flooding and ends flight before destruction,
so a submerged, stopped aircraft does not retain a spurious forced-descent cursor.
This differs from the forced-fall event: ordinary water contact is not a blanket
flooding rule added to every fall. Tests cover clear flight, landing, ground impact,
water contact, replay, section/dice preservation and nested-effect rollback.

Horizontal control updates, fuel accounting, ordinary-flight scheduling, landing
mines, character publication and live aircraft admission still need host integration.
The existing forced-fall dispatcher remains separate from ordinary flight movement.

### Launch and ordinary flight in movement ticks

Movement dispatch now advances launch countdowns and active powered flight after
existing forced descents. Launch rechecks power, rotor, fuel and underground
restrictions. Flight consumes fuel before controls and contact resolution; newly
exhausted fuel starts a descent whose clock first advances on the next tick.
The host supplies `nofusionvtolfuel` through movement rules. Vertical-only motion
and launch countdowns keep the server tick active.

Vehicle heading, turning slowdown and acceleration are now separable from position
projection. Ground vehicles retain their terrain effects, while airborne VTOLs use
the same controls without terrain slowdown, then the shared flight path resolver.
Horizontal commands use aircraft velocity and tail-rotor limits. Airborne shutdown
starts the shared descent immediately; shutdown on the surface cancels launch.

Contact results enter existing movement publication: landing mines, crash reports,
water-destruction notices and ceiling feedback. Tests cover exactly-once movement,
restartable launch/fuel transitions, fusion fuel exemption, ceiling rechecks,
control limits and shutdown. Unlinked map-edge handling and native flight controls
are described below. Live aircraft admission, wrapped maps, powered recovery,
wreck descent and full character consequence admission remain unfinished.

### Native and Lua flight controls

Native `takeoff [delay]` and `vertical [kph]` commands now share domain controls
with `btech.unit.takeoff` and `btech.unit.vertical`. They use the same operator,
power, fuel, rotor and velocity checks. Nonzero launch-delay overrides require
wizard authority. Both adapters stage the same feedback and roll back state and
messages on failure; vertical readout is also available without mutation.

The existing `land` command and Lua method dispatch to VTOL touchdown or queued
launch cancellation as appropriate. Deliberate and automatic aircraft landings
share one consequence collector for notices and landing mines, with existing
movement publication responsible for character injuries and casualties. Tests
cover adapter equivalence, permissions, invalid velocity, callback rollback,
launch cancellation and tactical touchdown. The host landing test verifies the
remaining live-aircraft gate restores state rather than publishing partial effects.

Flight help and Lua declarations document the commands. This does not remove the
live construction/state gates: wrapped-map flight, powered recovery, wreck descent
and remaining live-state validation still need completion before full admission.

### Unlinked aircraft map boundaries

Flight now resolves an unlinked map boundary at the first traced exit, placing
the aircraft at the last in-bounds hex center. It stops actual and requested
horizontal speed through the same movement helper used by Mechs and ground
vehicles, preserving the aircraft's requested heading and vertical speed.
Altitude is interpolated at the exit; subsequent ticks continue climb or descent.
Fuel and control updates remain part of the enclosing movement transaction.

Earlier terrain contacts take precedence over map exits. Tests cover all eight
compass directions, saved-state replay, continued vertical motion, fuel accounting
and an intervening hill collision. Wrapped maps, transitions between maps and
building exits remain unfinished; this boundary handling does not admit live VTOLs.

### Powered recovery during forced descent

Aircraft recovery now uses the shared forced-descent cursor. Sustained lift reduces
downward speed on each three-second event; reaching zero retains one scheduled
event before returning to airborne flight. Losing lift during that interval resumes
acceleration. Surface contact can still cause an impact while braking.

The aircraft adapter checks running power, intact rotor, surviving unit and fuel,
including the configured fusion fuel exemption. Movement dispatch passes that
configuration through the same recovery path. Recovery itself spends no dice and
applies no impact damage; normal flight controls and fuel accounting resume through
ordinary movement. Tests cover saved replay at every second, interrupted arrest,
contact during braking, fuel and rotor restrictions, and dispatcher integration.
Wreck descent and live-aircraft admission remain unfinished.

### Destroyed aircraft descent

Hull destruction retains the shared forced-descent cursor. Destroyed aircraft now
advance that cursor and settle through the same crash resolver as surviving
aircraft. Fall direction, terrain damage scaling and impact packets use the shared
vehicle fall implementation, including follow-up hits against remaining wreck
sections. Destruction has already released the pilot; settlement does not invent
another assigned-pilot injury or revive the unit.

The ordinary vehicle-fall entry point still rejects wrecks. Aircraft crash callers
own admission to the shared material resolver. Tests cover ground, water and bridge
settlement, save/load at each tick, movement-dispatch equivalence and exactly-once
impact. Existing ice and mine eligibility rules remain shared. Full live VTOL
admission and character-consequence validation remain unfinished.

### Live VTOL admission

VTOLs now use ordinary vehicle creation, placement, cockpit assignment, startup
and host movement actions. The blanket construction and world-state gates are
removed. World validation checks active flight placement, rotorcraft locomotion,
power and rotor integrity, forced-descent vertical state, and separation from
stored ground-vehicle elevation. Touchdown retains altitude in flight state.

Native and Lua flight controls are exercised against fully registered aircraft.
Tests verify host landing, climb and shutdown descent, database save/load during
flight, deterministic settlement after restart, invalid lifecycle rejection and
character injury publication after rotor loss. Incomplete component fixtures still
fail ordinary world placement checks and preserve transactional rollback.

This admits the implemented flight lifecycle; it does not claim completion of
wrapped maps, building transitions, cargo operations or the remaining broader
BattleTech coverage work.

### Saved opposite-edge map wrapping

Type-7 linked-map markers now restore opposite-edge wrapping. Ground Mech movement,
ground-vehicle movement and powered VTOL traversal share one virtual-coordinate
resolver. Paths are traced before coordinates wrap, preserving seam-entry hazards
without inventing a path through the map interior. A crossing ends at the resolved
hex center and retains speed. Aircraft contacts use the same coordinate mapping
before the shared landing or crash resolver runs.

`set_battle_map_wrapping` changes the saved policy without moving units. Persistence
preserves existing linked-marker payloads on unchanged saves and removes only those
markers when wrapping is disabled. Terrain reload retains the policy. Tests cover
both horizontal directions, all three movement families, database replay, an aircraft
collision at the opposite edge, and imported marker preservation.

This covers ordinary movement within one map. Jump trajectories, building exits,
transitions between separate maps and a dedicated administration command remain
separate integration work.

### Wrapping administration

`@btech map-wrapping <map>=<on|off>` now configures the saved policy through the
existing Wizard map-administration path and target-control checks. Lua exposes
`btech.map.wrapping(map, enabled)` inside callback transactions; `enabled` must be
a boolean. `btech.map.inspect(map).wrapping` reports the current setting.

Both adapters call the same domain setter. Tests verify native/Lua equivalence,
non-Wizard rejection, invalid input, callback rollback and disabling the policy.
Help and Lua declarations document the controls. Jump wrapping and transitions
between different maps remain separate work.

### Wrapped jump trajectories

Projected jumps can now cross wrapped map edges. Launch and saved-route validation
look up virtual coordinates through the shared boundary resolver. The saved flight
cursor retains the dimensions used for its last resolved sample while preserving
its original trajectory, distance and altitude calculation. Movement refreshes the
boundary policy before advancing; world validation rejects mismatched dimensions.

Interrupted ice-crossing movement passes its unwrapped segment into the same ground
traversal resolver, avoiding a false route across the map interior. Disabling wrapping
is rejected atomically while an active jump requires it. Tests cover all four edges,
normal unlinked rejection, increasing flight distance, save/load after crossing,
wrapped landing positions and malformed saved dimensions. Existing jump integration
checks also pass. Transitions between distinct maps and building exits remain
separate from opposite-edge wrapping.

### Shared building route definitions

Interior arrival points (map-object type 6) and return-map links (type 5) now have
ordered Rust representations. Entry lookup selects the first exterior entrance,
then the first matching interior direction; an omitted direction selects the first
point. Exit lookup selects the first return map and its first reciprocal exterior
entrance. Missing or invalid routes return an error without moving units.

The same route lookup is available to every admitted unit class. Selective persistence
owns only arrival coordinates/direction and return-map identity, preserving auxiliary
columns. Terrain reload retains routes; map purge removes inbound return links.
Validation checks bounds and map references. Tests cover selection order, direction
matching, invalid configuration, database replay, selective updates and reload.

These destination definitions feed the entry lifecycle and host transaction below.
Carried-unit handling remains outstanding; map-edge dispatch is described below.

### Shared powered map transfer

Administrative placement and powered transfer now use one placement implementation
for map membership, coordinates, contact cleanup, network detachment and world
containment. Administrative placement retains its shutdown requirement and resets
motion. `transfer_battle_unit` preserves running speed, requested speed, heading,
requested heading and cockpit ownership; it clears stale targeting and assigns the
first vacant destination slot. Landed VTOL altitude follows the destination surface.

The transfer transaction rejects airborne/pending-fall units and standing attempts,
and validates the resulting BattleTech world before committing. Post-jump
stabilization and stagger damage survive a powered transfer; administrative relocation
resets them. Tests cover Mechs, ground vehicles and
landed VTOLs, material/dice preservation, slot allocation, database replay, rejected
destinations and pending-flight rejection. Building actions own route admission,
timers, locks and movement publication; carried-unit handling remains outstanding.

### Shared building entry event state

Mechs, ground vehicles and VTOLs now save the same `BattleBuildingEntry` record in
their owned snapshots. Admission checks the current pilot, power, grounded posture,
VTOL fuel, speed and route. A denied enter lock can be forced only when the building
is unsafe and its integrity is strictly below integer half of maximum integrity.
The host supplies the lock decision; domain code does not evaluate Lua locks.

An admitted event retains a normalized direction and an eighteen-second countdown.
It does not reserve slots or retain a stale destination. The explicit entry clock
returns readiness and retains the ready event until the host consumes it in a
movement transaction. The host must resolve the current route and eligibility and
retest the lock at that point. Save/load preserves both partial and ready events;
malformed countdowns are rejected. Duplicate scheduling and failed admission leave
state unchanged.

The host transaction below connects this lifecycle to the server clock, locks and
movement callbacks. Native and Lua entry commands share that action. Carried-unit
handling remains outstanding. Cargo speed adjustments remain part of the outstanding
cargo implementation. Integration tests cover the
shared lifecycle across all three included chassis, persistence, route changes,
lock boundaries, speed rejection and invalid saved countdowns.

### Building entry host transactions

The server now advances pending entries, including those owned by stationary or
shut-down units. `begin_battle_building_entry_action` evaluates the enter lock and
stages door feedback. At expiry the host rechecks eligibility, resolves the current
route, evaluates the enter lock with the unit as enactor, and uses the shared
teleport movement transaction. Invalid eligibility or denied policy consumes the
event. Callback failure restores the countdown and all staged effects for retry.

The ordinary movement transaction has a relocation hook between departure and
arrival callbacks. Building entry uses it to update battlefield membership and
coordinates together with containment; arrival callbacks see consistent placement.
Ordinary movement retains its existing behavior. Tests cover all included chassis,
arrival-callback rollback, retry, changed enter locks and persisted completed moves.

Native `enterbase [direction]` and Lua `btech.unit.enterbase(unit, pilot, direction)`
share the same text adapter and host action. A single-byte selector chooses a
direction; omitted or longer selectors choose the first entrance. Multiple arguments
are rejected. Lua returns true for scheduling and false for enter-lock denial;
callback failure rolls back the event and its messages. Tests compare both adapters
for Mechs, ground vehicles and VTOLs. Carried-unit batches remain outstanding. The earlier explicit entry clock is now called by the
server host action; callers must not also advance it independently in the same tick.


### Building entry visibility and names

Opening doors use the shared hex-visibility broadcast, describing the current
coordinate as “your hex” for occupants there. Observers do not need an acquired
unit contact to see the doors. Successful entry names the authored structure,
using the same naming helper as building damage. Structure action names do not
invoke the contact-report identification lock.

Departure notices capture current acquired contacts before map transfer clears
them. They name the structure and exterior coordinate, and are published only
after successful movement callbacks. Unknown units do not become identified just
because their hangar doors are visible. Tests cover these audiences with Mechs,
ground vehicles and VTOLs, alongside callback rollback. Interior-arrival audience selection follows the ordinary destination-map scanner
pass, as described below.


### Interior arrival publication

The entry host action returns transient `BattleBuildingArrival` reports for the
surrounding tick. New map membership extends that tick's eligible scanner observers
before startup completion. The existing scanner pass deduplicates observers and
performs acquisition once. Reports are then published to currently acquired, visible
contacts on the destination map; a unit which has moved away or been replaced is
ignored. Publication does not roll dice, grant experience or mutate contact state.

These reports are not independent saved events: discard them on tick rollback and
regenerate them when the restored entry event is retried. Standalone host callers
must run their normal scanner pass and call `publish_battle_building_arrivals` with
the returned reports before committing the enclosing transaction. Tests cover all
included chassis, newly eligible interior observers, no disclosure before acquisition,
unchanged state during publication and stale-report suppression.

### Shared building-exit placement

`exit_battle_building` resolves the first return link and reciprocal exterior
entrance, rejecting rubble-filled exits before mutation. It reuses the common
placement implementation for slots, contact cleanup, network detachment and
containment. Crew, requested controls and combat material survive; forward speed
is capped at the destination's effective maximum. VTOLs continue airborne one
level above the exterior surface, preserving vertical speed and consuming no fuel
or extra takeoff delay. Stable airborne VTOLs are admitted by exits; ordinary
powered transfers still require them to land.

The candidate world is validated before commit. Tests cover all included chassis,
rubble rejection, destination slots, control/material preservation, destination
speed adjustment, landed/airborne VTOL continuation and database replay. This is
the exit placement operation. The shared host action below supplies teleport
callbacks and observer reports. Map-edge dispatch is described below; carried-unit
batches remain to be connected. Airborne Mech
transfers are not yet supported by this operation.


### Shared entry and exit host actions

`exit_battle_building_action` and delayed entry now use one host traversal function.
It evaluates teleport policies, rechecks the route after departure callbacks, commits
placement before arrival callbacks, and stages cockpit and source-observer feedback.
Both directions return the same opaque arrival report for publication after ordinary
sensor acquisition. Exit reports use the reciprocal exterior entrance coordinate.

Denied exit teleports leave placement unchanged and report the denial to cockpit
occupants. Callback failures restore placement, material, callback mutations and
staged feedback. Tests cover successful replay, denied teleports, callback rollback
and exterior-observer publication for every included chassis. The implementation
lives in `building_actions.rs`; no separate vehicle host workflow is introduced.
Automatic map-edge dispatch is described below; carried-unit batches remain outstanding.


### Automatic building exits at map boundaries

Ground Mechs, ground vehicles and airborne VTOLs now report attempted unlinked
boundary crossings to one host dispatcher. Material movement first retains a valid
stopped position and the running controls. A configured building exit invokes the
shared host traversal. Success restores controls for transfer, removes the fallback
edge-stop notice and returns an arrival report. Missing or rubble-filled routes and
denied teleports leave the unit stopped. Callback failure restores the whole movement
step, including fuel, dice, contacts and notifications.

Wrapping takes precedence over building exits. Ordinary maps keep their existing
boundary behavior. Tests cover all four edges across the included moving chassis,
rubble and teleport denial, callback rollback, wrapping precedence and database
replay. Movement host actions now return building arrival reports; the server merges
them into the ordinary scanner/publication pass, preserving the startup scanning
delay. Material-only movement APIs retain boundary stops because they cannot execute
host policies or callbacks. Jumping-Mech transitions and carried-unit batches remain outstanding.

Map transfers and hangar admission share airborne-state validation. Mechs, ground
vehicles and VTOLs must finish forced descent before entering or leaving another
map; transfer cannot erase a pending fall. Building exits still admit stable VTOL
flight. Regression coverage checks rejection without changing position or fall
state after a database restart for all three chassis families.


### Unpiloted in-character exit warnings

Successful in-character exits warn cockpit occupants when no operator remains
assigned. The intruder and self-destruct announcements follow the structure-exit
message, share its transaction, and do not occur for denied or failed exits.
Out-of-character units and occupied assigned cockpits do not warn.

The reference shutdown routine returns immediately when its pilot is unassigned;
it does not schedule destruction. Rust clears pilot assignments on departure and
rejects saved assignments to players outside the unit. Accordingly these reachable
unassigned-cockpit exits preserve power, material and dice after warning rather than
inventing a destruction timer or forced shutdown. Tests cover the character/assignment
matrix for all included chassis and verify callback rollback and unchanged material.

### Shared external tow ownership

`set_battle_tow` establishes or detaches a prepared carrier–target pair. One map in
`BtechState` owns the relationship for Mechs, ground vehicles and VTOLs; reverse
lookup is derived. This API is a relationship primitive, not the player pickup
command. Attachment requires distinct constructed units in the same hex and an
already powered-down target. Pairs cannot overlap or form chains. Startup rejects
a towed target, and administrative placement requires detachment at either end.

Saved pairs must remain on the same battlefield. Deferred object destruction can
retain the link until maintenance; purging either endpoint or its map releases it.
The optional Rust-owned `btech_tows` table updates only owned columns and preserves
additional columns when partners change, including when two carriers swap targets.
World validation enforces pair disjointness before persistence and during loading.
Tests exercise every included carrier/target chassis combination, corrupt state,
partner swaps, startup/placement guards and unit/map purge replay.

Pickup authorization, salvage equipment and arm requirements, target preparation,
dropoff consequences, coordinate following, towing speed effects and atomic paired
building transfers are not implemented by this primitive. These must use the common
relationship state when their gameplay actions are added.

### Tow position following

The common movement completion step mirrors each carrier's committed hex, continuous
point, actual speed, heading and integer elevation onto its tow target. Ground and airborne
movement use the same operation. Only the final storage assignment distinguishes
Mech placement from vehicle placement and VTOL flight altitude. Slots, occupants,
material and target dice remain unchanged. Interrupted Mech hex transitions wait
for their tactical position to commit before the tow follows.

Targets keep their engines and movement controls stopped while actual speed follows
the carrier, including reverse travel and speeds beyond the target’s engine limit.
Chassis validation checks self-propelled motion; world validation requires a tow
relationship before accepting externally imposed unpowered motion. No second tow
flag or carrier reference is stored in the chassis. Detachment clears inherited
speed and throttle in its validated candidate. Flight
altitude is currently projected at integer elevation and must fit the retained
16-bit elevation field. A towed powered-down Mech may retain height above ordinary
terrain levels; untowed Mechs retain the normal terrain-height validation. Raw
detachment validates its candidate and cannot leave an invalid elevated unit;
player dropoff still needs its landing/fall consequences.

Attachment rejects targets with independent airborne movement. Mechs reject jump
launches while carrying another unit. Cross-chassis tests cover continuous position,
facing, high-altitude carriage, unchanged slots/dice, persistence replay and jump
rejection. Tests also cover immobilized targets, forward and reverse speed beyond
target limits, save/replay, invalid unlinked motion and detachment braking.
Pickup policy, dropoff falls,
terrain effects along the tow path, and atomic paired building transfers remain
unfinished.

### Shared load accounting

`battle_unit_load(world, unit, tsm_tow_bonus)` returns a derived `BattleUnitLoad`
for any included chassis. Material mass uses surviving components and current
ammunition. External tow mass starts at twice the target's live material mass;
salvage equipment, enabled hot-myomer towing assistance and carrier equipment each
halve that integral load in sequence. No weight cache or extra tow ownership state
is stored. Mech construction accepts the salvage and carrier metadata used here.

`BattleUnitLoad::maximum_speed` applies the common construction-weight surcharge,
carried-load denominator floors and strict three-times-nominal overload threshold.
It uses the fixed mass units and single-precision speed calculation shared with
the existing unit-local Mech effective-speed query. Destroyed carriers return zero;
invalid inputs and arithmetic overflow return errors. Boosters, gravity and other
movement rules apply after this base load calculation.

Tests cover all chassis pairings and salvage/carrier combinations, live ammunition
changes and replay, the configured myomer heat threshold, integral discount rounding,
overflow and exact overload boundaries. World movement updates consume this calculation. Native and Lua speed commands and Lua throttle readouts use the loaded limit.
Other effective-speed consumers still need to be connected. Container inventory cargo also remains outside
this query until its gameplay subsystem is implemented.


### Loaded horizontal movement

Mech ground movement, vehicle driving and VTOL horizontal flight now use the shared
load calculation when carrying a tow. It applies to the damage-adjusted base before
Mech myomer and booster conversions. Acceleration, turning and quad lateral penalties
use the loaded ceiling; vehicle road and bridge bonuses retain their paved-surface
headroom. Existing unit-local movement calculations remain available for isolated
chassis simulation without world-owned towing relationships.

Each movement update reconciles actual speed and throttle against the current load.
An overloaded carrier stops horizontal translation and turning without damaging its
propulsion. The carried unit follows the resulting actual speed. Load is recalculated
from live material, with no invalidation cache. The server and configured landing
rules supply `tsm_tow_bonus`; standalone movement rules default it to enabled.

Cross-chassis tests verify load ceilings, overload stops, forward/reverse acceleration,
carried speed, and database replay. Remaining native status/effective-speed consumers, pickup/dropoff consequences
and paired map transfers remain outstanding.


### Loaded speed commands and Lua reports

`battle_throttle_maximum` supplies the world-aware command ceiling for all admitted
chassis. Native and Lua speed requests use it for walking/cruise, running/flank,
reverse and numeric clamping, then use the same configured admission path. Both
adapters honor `tsm_tow_bonus`; isolated `set_battle_speed` uses the standard enabled
setting. No loaded flags or limits are copied into chassis state.

Lua Mech `movement_maximum_speed` and vehicle `maximum_speed` now report this same
loaded ceiling. VTOL horizontal requests additionally allocate the loaded velocity
budget against current climb/descent and apply tail-rotor cruise safety. Vertical commands also use this loaded budget. Some native status/effective-speed
reports remain outside this query.

Tests compare native and Lua command outcomes across chassis and speed aliases,
check numeric bounds and Lua reports, exercise configured myomer assistance around
the activation threshold, verify simultaneous VTOL climb/horizontal allocation,
and confirm callback rollback restores state and suppresses notices.


### Loaded vertical command admission

VTOL climb and descent commands use the world’s loaded throttle ceiling and the
same perpendicular-speed calculation as horizontal requests. The requested vertical
magnitude must fit the budget remaining after the current horizontal throttle.
An overloaded VTOL therefore accepts zero vertical speed and rejects nonzero requests.
Power, crew, fuel, rotor and airborne-state checks remain in the common vehicle
control implementation used by both isolated and world-aware callers.

Native and Lua commands share admission and notifications. Tests cover both vertical
directions with ordinary and overloaded tow loads, exact state preservation on
rejection, callback rollback without notices, and movement replay after saving.
This governs command admission; changing a load does not itself issue a new vertical
command or model cargo-related lift loss. Pickup/dropoff and paired map transfers
still need their gameplay consequences.

### Loaded Mech status reports

Native and Lua Mech status reports and observer scans now obtain throttle limits
from `battle_throttle_maximum`, sharing the calculation used by movement commands.
The host supplies its configured myomer towing assistance. Isolated report APIs
use the standard enabled setting. Mechanical maximum remains a separate report
of chassis capacity; conventional carriers also show their throttle maximum with
a tow, and myomer-equipped Mechs show the configured limit in their myomer line.

Tests compare native, Lua and observer reports on either side of the myomer heat
threshold with assistance enabled and disabled, check that reports leave state
and notifications unchanged, and verify conventional carrier reports after saving
and reloading. Vehicle cockpit status and other effective-speed consumers remain
outstanding, along with pickup/dropoff consequences and paired map transfers.


### Ground vehicle and VTOL cockpit status

The native `status` command, Lua `btech.unit.status`, and observer scans now support
admitted ground vehicles and VTOLs. They share section selectors and precedence,
power wording, motion controls, sensor and lock timers, electronic-field reporting,
weapon readiness formatting, and compact ammunition grouping with Mechs. Vehicle
anatomy stays in its own renderer: hull protection, turret condition, crew state,
immobilization, flooding, and rotorcraft phase, altitude, vertical speed and fuel.
Loaded throttle limits use the same world query as commands. Compact exports use
mechanical capacity, and vehicle cooling defaults share the construction query.

Ordinary scans retain limited information; observer scans reuse cockpit sections.
Reports do not change simulation state. Tests exercise native/Lua parity, section
selection and precedence, observer disclosure, current ammunition losses, loaded
speed, continuous VTOL altitude, fuel and save/reload. This closes the vehicle
cockpit-status gap described above. Pickup/dropoff consequences, paired transfers
and the remaining effective-speed consumers are still outstanding.


### Paired building transfers

Building entry and exit now move a carrier and its external tow in one placement
transaction. Both units receive destination membership slots, lose stale contacts,
and keep their crew and tow relationship. The target mirrors the carrier's final
motion and height, including the VTOL exit lift above the exterior surface.
An unavailable target or invalid destination rolls back the entire placement.
A target cannot transfer independently while being towed.

Host transfers run both participants through the existing teleport-policy and
movement-callback pipeline. Departure callbacks precede the shared placement;
arrival callbacks run for the target and then the carrier. Denial or callback
failure discards all staged changes and effects. Both units contribute departure
and deferred arrival observer notices; arrival publication still waits for the
ordinary sensor pass and validates each participant's generation and destination.

Tests cover all nine chassis pairings through native/Lua entry and host exit,
both participants' callbacks, denial of either participant, failed placement,
callback rollback and database restart. Pickup/dropoff gameplay consequences and
remaining load-aware effective-speed consumers are still outstanding.


### Loaded building speed checks

`battle_effective_maximum_speed` derives the world-aware speed used by building
transfers. Mechs reuse the unit-local mass, booster, myomer and environmental
calculation with external load included. Vehicles reuse their damage-adjusted
loaded movement ceiling. No effective speeds are stored or cached.

Entry admission, the delayed event, and lock/departure callback rechecks all use
the configured `tsm_tow_bonus`. Entry still requires actual speed below one fifth
of effective maximum in either direction, retaining the existing exception below
one movement point. Exit applies its existing forward-speed cap using the loaded
maximum on the destination map, then synchronizes the tow. Desired controls and
vertical speed retain the existing exit behavior.

Tests exercise native/Lua rejection and delayed cancellation across all admitted
chassis, live and reloaded paired exits, and enabled/disabled hot-myomer assistance.
Standalone APIs use the standard enabled setting. Pickup/dropoff gameplay and
load-aware experience/battle-value consumers remain outstanding.


### Loaded gunnery experience and battle value

Classic gunnery experience now obtains both participants' effective speeds from
`battle_effective_maximum_speed`. World-aware `battle_unit_value` supplies current
load to the existing Mech and ground-vehicle valuation formulas; battle-value XP
uses those valuations. Its separate nominal-speed ratio remains nominal, as the
formula requires. Installed weapon value and ammunition accounting are unchanged.
Vehicle cooling uses the shared construction capacity query.

Firing adapters pass `tsm_tow_bonus` through `BattleShotRules` into each pre-impact
`BattleGunneryAwardRequest`, so grouped damage uses the configured assistance and
current load. Standalone requests explicitly choose the setting. No load or battle
value cache is introduced. Unit-local BV methods remain useful without world tow
relationships.

Tests cover towing by either participant in all Mech/ground-vehicle pairings,
classic difficulty and BV changes, configured hot-myomer assistance, read-only
valuation and replay after reconnecting the pilot. VTOL classic experience uses
the shared speed query; VTOL battle value still needs its flight valuation rules.
Pickup/dropoff gameplay remains unfinished.


### VTOL battle value

VTOLs now use the common vehicle valuation pipeline. Rotorcraft share the 30%
defensive reduction with hovercraft and add one movement-modifier point after
the common speed bands. That class bonus remains when landed or stopped by rotor
damage. Armor, structure, cooling, installed weapons, ammunition and world-owned
towing load use the existing shared queries; flight altitude and vertical speed
do not independently change BV.

The battle-value experience path now accepts VTOL attackers and targets, including
mixed Mech/ground-vehicle encounters. Tests check exact intact, unarmored-rotor and
destroyed-rotor scores, unchanged installed offense after expenditure, loaded XP,
pilot adjustment and save/reload. The asset valuation check includes all 34 shipped
VTOL templates. This closes the VTOL BV gap noted above; pickup/dropoff gameplay
and other outstanding integration work remain.


The broader asset check also identified an unnecessary construction rejection for
uncatalogued vehicle engine ratings. Material accounting now retains zero engine
mass for those ratings, as the reference does; the shared engine diagnostic still
reports the missing catalogue entry. This applies to all vehicle movement types,
without an asset-specific exception. `SalvageVTOLII` retains its authored 860
lookup rating and 161.25 maximum speed and now constructs, values and reloads.
This supersedes the catalogue-entry restriction noted in the stationary-construction
section above. Invalid numeric or equipment data continues to fail validation.


### Shared pickup admission and scenario permission

`battle_pickup_admission` checks modeled pickup prerequisites without changing
motion, power, dice, contacts or tow ownership. It shares cockpit authorization,
visibility, elevation and physical actuator queries. Common rules cover same-hex
placement, relative height, bridge separation, carrier technology, tow overlap,
hidden/burning/falling targets, team restrictions and the one-KPH pickup limit.
Bipeds need both arms and one operational shoulder/hand pair; quads cannot pick
up units. Vehicles and VTOLs need salvage equipment, with VTOL vertical motion
subject to the same one-KPH limit.

`set_battle_towable` is a trusted scenario edit for the out-of-character target
permission. The flag defaults false and persists in each construction record;
in-character targets do not require it. No separate relationship index is added.
Tests cover all nine chassis pairings, permission replay, authority, speed and
height boundaries, hidden/enemy targets, overlap, arm failures and missing salvage
gear. The Jenner correctly fails the hand requirement; Atlas fixtures exercise
successful Mech admission.

This is the admission layer, not the complete pickup command. Target preparation,
shutdown, prior-tow release, terrain consequences, notifications and native/Lua
pickup/dropoff actions remain unfinished. Fortification and other unimplemented
stances will also need admission gates when their state is introduced.

### Shared pickup target preparation

`prepare_battle_pickup` rechecks pickup admission and prepares the target on an
atomic candidate. It stops translation before shutdown, cancels pending building
entry, and makes a Mech prone with centered torso, normal arm facing and no stand
timer. VTOL preparation cancels launch and vertical movement. Targets that are
starting or running use the same shutdown mechanics as ordinary cockpit shutdown,
including electronics, target locks and pilot release. Pickup authorization comes
from the carrier, so an uncrewed target does not need a fictitious operator.
Ordinary shutdown retains its own pilot authorization check.

Tests cover all nine chassis pairings in off, starting and running states,
uncrewed targets, moving-target arrest without damage or random draws, rejected
admission rollback, subsequent attachment and persistence replay.

This preparation API is a component of the enclosing pickup transaction. It does
not attach cables, release a target's previous tow, synchronize carried height,
apply terrain effects, or publish player pickup notifications. Those operations
and native/Lua pickup/dropoff commands still require integration.

### Ground-vehicle forced descent

Ground vehicles now retain `BattleFreeFall` when support is removed, using the
same three-second descent clock, increasing downward speed and impact severity
as rotorcraft. `begin_battle_vehicle_descent` starts at the retained vehicle
height after tow detachment; `advance_battle_vehicle_descent` advances either
class through one common transaction. The common outcome is
`BattleVehicleDescentEvent`. The aircraft-specific advance entry still requires
a VTOL. Aircraft retain their lift-recovery and rotor-impact consequences;
ground vehicles use the existing vehicle fall material rules.

The normal movement heartbeat advances both classes once, publishes through the
existing vehicle fall report path, and excludes falling ground vehicles from
ordinary driving. Ground descent rejects concurrent terrain altitude, missing
placement/motion, aircraft state or active translation. Administrative placement
clears descent. Pickup and raw tow attachment reject falling vehicle targets.
Tests cover shared cadence, impact severity, persistence replay, duplicate-start
and on-surface rejection, malformed snapshots and heartbeat advancement.

This supplies the descent mechanism needed by elevated dropoff. Tow release,
Mech wreck descent, terrain settlement and player pickup/dropoff command assembly
remain unfinished.

### Shared tow release and Mech wreck descent

`release_battle_tow` atomically detaches the target and stops its external motion.
Within two levels of support it settles the target; above that it starts the
existing Mech or shared vehicle descent clock. Ice settlement uses the ice
surface, and bridge settlement uses the existing deck/underpass surface query.
The operation returns cockpit and visibility-filtered interaction notices for
publication by its caller. It is a trusted domain operation so both operator
release and pickup of a target carrying its own tow can reuse it.

Mech descent now survives destruction during a fall and permits a wreck released
from height. Impact uses the existing fall consequences; a destroyed Mech does
not acquire a jump-stabilization timer. Tests cover all nine carrier/target
pairings at heights two, three and twelve, unavailable-target rollback, missing
tow rejection, persistence replay through impact, and destruction before release
or during descent. Existing jump and casualty rollback tests also pass.

Player command admission/publication, pickup attachment and terrain consequences
remain to be assembled into the native/Lua actions. This release operation does
not replace those host transactions.

### Pickup and dropoff command integration

Native `pickup <label|#dbref>` and `dropoff`, and Lua
`btech.unit.pickup(carrier, pilot, target)` / `btech.unit.dropoff(carrier, pilot)`,
now call `battle_tow_action`. The action checkpoints world state and effects,
composes the shared material operations, publishes nested fall/ice consequences
and new casualties, then validates the complete world. Lua callers use the usual
mutation context; scripts retain responsibility for authority to act for the
specified pilot. Dropoff checks the carrier's conscious assigned operator.

Pickup rechecks admission, releases a tow owned by the target, prepares and shuts
down the target, attaches one disjoint pair, mirrors motion/height, applies
flooding, resolves upward or downward ice breakage, and corrects loaded speed.
The upward ice-break helper now supports vehicles through the same surface-change
implementation as Mechs. Observer notices use the shared visibility-filtered
interaction formatter. Shutdown, landing and towing share configured fall rules.
`pickup_battle_unit` exposes the tactical material operation; character effects
require the host action. Scenario towing permission remains available through
the trusted Rust setter.

Tests cover previous-tow release across all nine pairings, native/Lua parity for
in-character participants, Lua rollback after pickup and dropoff, downward ice
breakage for Mech/ground carriers, upward breakage for VTOL/hover carriers, and a
late ice-effect rejection that restores the prior tow, terrain, material and dice.
Piloting help and Lua declarations document both commands.

Fortification and hull-down/dug-in state still need their corresponding admission
gates when those states are implemented. Broader carried-height precision and
container cargo remain separate unfinished integration work.

### Scenario towing permission and common inspection

`@btech unit-towable <unit>=on|off` now exposes the scenario flag through the
wizard command boundary. Trusted Lua scripts can call
`btech.unit.towable(unit, enabled?)`; omitting the second argument queries the
flag. These adapters reuse `set_battle_towable` and its existing persistence.
Permission changes affect future out-of-character pickup admission and do not
alter an established relationship.

`@btech inspect #unit` reports permission, current target and current carrier for
all supported chassis. Lua `btech.unit.state` builds `towable`, `towing`,
`towed_by` and `free_fall` once before projecting chassis-specific fields. This
also makes ground-vehicle descent visible without inspecting private storage.
The Lua declarations share a transport-state base type.

Tests cover native wizard admission, native/Lua state equivalence, permission
rollback, disabling permission during an active tow, invalid option rejection,
persistence replay, both ownership directions, and read-only ground/VTOL descent
inspection. This closes the Rust-only scenario-permission limitation noted above.

### Ground-vehicle digging cover

Native `dig` and Lua `btech.unit.dig(unit, pilot)` share a transactional action
and a saved `BattleDigState`. Tracked and wheeled vehicles prepare for twenty
simulation seconds on eligible terrain. The ordinary heartbeat advances the
countdown; restart preserves remaining time. Movement and heading commands cancel
preparation. Completed cover blocks chassis turning and hull weapons until a
movement request exceeds 0.1 KPH. Shutdown cancels preparation while preserving
completed cover; pickup and placement clear cover.

Weapon reservation uses the existing readiness boundary, including all launchers.
Mech and vehicle aiming share the configured `digbonus`, `dig_only_fs` and hit-arc
checks through target modifiers. The common ground-vehicle impact router applies
the 41/42 percentage boundary before the selected hit table, using original turret
integrity for standard/FASA rules and surviving integrity for advanced rules.
No separate launcher, aim, or timer implementation was introduced for cover.

Tests cover native/Lua parity and rollback, timer persistence, unsupported chassis
and invalid saved countdowns, movement and shutdown transitions, pickup, shared
attacker modifiers, hull-weapon readiness, and seeded hit-table boundaries.
Hull-down and fortification remain unfinished.

### Quad hull-down posture

`hulldown [-|stop]` and `btech.unit.hulldown(unit, pilot, argument?)` use one
transactional action. Saved `BattleHullDownState` holds the completed posture and
a pending direction/countdown. The ordinary heartbeat owns advancement, including
restart recovery. Delay is the truncated value of 30 divided by maximum chassis
speed in MP, clamped to a divisor of 1–30. Lowering requests zero desired speed.
The reference admission checks forward speed above 0.5 KPH; this preserves its
signed reverse-speed behavior.

Shared movement admission blocks heading, speed, standing and jumping while
lowered or changing. Shutdown/destruction cancel pending changes; falls, pickup
and administrative placement clear the posture. Native/Lua notifications and
visibility-filtered observer messages share a rollback boundary. Status, contact
rows and detached Lua state expose the stance.

A shared cover helper adds two to visual, light-amplification, seismic,
electromagnetic and radar aim only when terrain LOS already supplies partial
cover. Infrared and probes keep their existing modifiers. This changes sensor
aiming, not hit tables or launcher logic. Tests cover timers, cancellation,
restart and rollback, movement gates, lifecycle cleanup, chassis admission,
invalid saved state and sensor exceptions for Mech and vehicle observers.
Fortification and the other documented integration gaps remain unfinished.

### Shared scenario fortification

`@btech unit-fortified <unit>=on|off` and trusted Lua
`btech.unit.fortified(unit, enabled?)` share one scenario setter and inspection
query for Mechs, ground vehicles and VTOLs. The flag is saved with construction
state and appears in detached Lua state, cockpit information and wizard inspection.
It survives shutdown and restart. Enabling requires settled motion, no tow pair,
no pending building entry, and a landed unit; disabling does not resume actions.

One admission helper blocks speed, heading, jump, vertical motion, takeoff,
digging, hull-down changes, hangar entry and both sides of pickup. Raw towing and
saved-pair validation reject fortified participants too. Material VTOL controls
use the same restriction. Fortification contributes the ordinary immobile-target
adjustment to ranged and physical aim, without stacking a second adjustment on
an already shut-down target. It does not introduce armor bonuses, weapon gates,
repair behavior or a new launcher path.

Tests cover native/Lua parity, wizard permissions, rollback, persistence and
inspection for all four chassis categories; movement/stance admission; immobile
aim and shutdown; every Mech/ground/VTOL tow pairing; and rejection of queued
movement or takeoff while enabling the setting. This closes the fortification
gap noted in the stance and pickup sections above. Other documented integration
gaps, including transport precision and remaining movement cases, remain open.

### Continuous altitude through external towing

Retained ground height now uses the same continuous terrain-level units as VTOL
altitude. It remains one optional physical height override per unit; no separate
transport-height cache or second relationship store was added. Shared towing
copies continuous altitude to any Mech, ground vehicle or VTOL target. Shared
range and LOS queries preserve these fractions, including ordinary VTOL flight.
`battle_unit_altitude` and Lua `btech.unit.state(unit).altitude` expose this value;
`battle_unit_elevation` retains the integer terrain-rule projection.

Release still uses integer height to decide whether the target is more than two
levels above its surface. A target at 2.99 settles; one at 3.75 begins descent.
The shared free-fall cursor retains 3.75 through its first two waiting seconds,
then the scheduled descent step moves from integer level 3 to level 1. This
matches the reference's event boundary without changing fall cadence or impact
severity. Retained and falling heights reject nonfinite/out-of-range values.

Terrain breakage preserves the continuous height present before changing the
surface. Ground movement and landing continue to establish whole-level support
heights. Saved retained heights and free-fall elevation are numeric rather than
integer-only fields. Tests cover all nine tow pairings, range, Lua inspection,
real vertical flight ticks, persistence/replay, fractional release thresholds,
shared descent timing and invalid height rejection. This closes the carried-height
precision gap noted above. Container transport and other documented movement
cases remain unfinished.


### Shared radio across supported chassis

Mechs, ground vehicles and VTOLs share channel settings, hardware capabilities,
analog interference, directed digital relays, frequency scanning, audit messages
and communication experience. Delivery visits all supported units in saved map
order. A small adapter borrows each chassis's radio storage and existing dice;
there are no separate vehicle delivery or relay algorithms.

Cockpit commands and Lua use the same controls while shut down. Transmission
rejects destroyed equipment and stunned operators. Startup captures communication
skill and resets the reception experience gate; the shared heartbeat advances the
gate even while shut down. Vehicle channel state is saved and exposed by
`btech.unit.state` alongside the same Mech fields.

Mixed-chassis regression scenarios rotate sender, relay and receiver roles,
checking relay paths, shutdown relays, analog interference, scanning, experience
cadence, native/Lua controls, restart, and rollback of both dice and command mines.
Autopilot radio replies remain outside the current scope.


### Mixed-chassis cockpit maps and navigation

Tactical maps, LRS and navigation now use the same position and contact projection
for Mechs, ground vehicles and VTOLs. Contact markers share battlefield membership
order and keep the observer's own marker visible when units stack. LRS uses
movement symbols `b/q/t/w/h/v` for bipeds, quads, tracked vehicles, wheeled vehicles,
hovercraft and VTOLs; friendly markers are lowercase and hostile markers uppercase.

The common centering rules accept mixed-unit contacts and retain hardware range
checks. Navigation includes VTOL vertical speed. Range and bearing use the same
mixed-unit endpoints, including continuous altitude for airborne or retained-height
units, without changing contacts or selecting a new target.

Regression coverage renders every included movement type, checks tactical and LRS
modes, friendly/hostile symbols, contact centering, VTOL readouts, range/bearing,
native/Lua equality and database replay. Electromagnetic terrain scans share the
mixed-chassis sensor rules described below.


### Observer roles across supported chassis

The saved observer role now applies to Mechs, ground vehicles and VTOLs through
the same scanner and radio projections. Wizard control is
`@btech unit-observer <unit>=on|off`; trusted Lua uses
`btech.unit.observer(unit, enabled?)`. Inspection reports the role for every chassis.

Observer scans expose exact material and bypass their normal unit-scan range;
projected map centers bypass the hardware radius. Acquired visibility and
contact-centered map range checks still apply. Observer radios receive untuned
traffic with clear sender metadata, targeted radio transmission is blocked, and
routine contact chatter is suppressed while lock-loss warnings remain.

Tests cover administration and Lua rollback, ordinary-player denial, disclosure,
range-policy distinctions, mixed radio reception, lock warnings, persisted replay
and disabling the role for each included chassis family.


### Mixed electromagnetic and seismic sensors

Mechs, ground vehicles and VTOLs use common electromagnetic and seismic queries,
scanner acquisition and terrain visibility. Vehicle signal fluctuation has its own
saved random stream, independent of attack dice. Completed weapon launches expose
the same transient electromagnetic signature, including misses; failed Streak
locks do not emit. The heartbeat clears emissions for every supported chassis.

Seismic detection requires landed VTOL observers and targets, and excludes hover
and stationary targets. Hover and stationary observers can detect qualifying
targets. Fixed installations extend hardware range by forty percent before signal
fluctuation; electromagnetic acquisition still ends where its chance becomes zero.

Tests rotate Mechs, ground vehicles and VTOLs through both roles, covering sensor
selection, terrain visibility, emissions, signal replay without attack-dice
consumption, airborne exclusions and fixed-installation range boundaries.


### Infrared across supported chassis

Infrared unit queries, acquisition and aiming accept Mechs, ground vehicles and
VTOLs through common visibility and thermal-contrast rules. Ground vehicles and
VTOLs do not run the normal thermal-production cycle; their weapon-heat storage
does not become infrared heat production. Mechs retain their existing production
and cooling calculation.

Fixed installations have a 21-hex infrared hardware range; mobile units retain
15 hexes. Unit and terrain queries share this limit and the battlefield ceiling.
Tests cover mixed chassis, secondary acquisition dice, underwater vehicles,
read-only aiming, stored vehicle weapon heat, database replay and range boundaries.


### Character vehicle terrain firing

Character-controlled ground vehicles and VTOLs can fire at coordinates through
the shared host firing action. The existing launch, terrain impact, injury and
evacuation handlers own these consequences; raw tactical terrain shots retain
their non-character restriction. No additional launcher or casualty path is used.

Tests cover every coordinate mode with ground vehicles and airborne VTOLs,
native/Lua equality, callback rollback and database replay. Seeded rapid-fire
misloads verify character injury, crew death and complete rollback when the
evacuation destination is unavailable. Indirect terrain firing is covered below.


### Occupied spotter coordinates

A conventional spotter may select a coordinate containing a unit. Shared target
resolution chooses the first live occupant in battlefield order, excluding the
firer, and rechecks the spotter's current contact. Mech and vehicle firers use the
same indirect shot path even when they cannot see the target themselves.

Coordinate spotting retains coordination, movement and lock costs but omits the
spotter's terrain aim modifier, sensor aim dice and spotting skill awards. Tests
cover all Mech/vehicle shooter-observer pairings, native/Lua firing, replay,
callback rollback, unchanged query state and a target leaving the coordinate.
Empty spotter hexes use the terrain-shot integration described below.


### Empty-coordinate indirect fire

Conventional indirect weapons can fire at an empty spotter coordinate through
the common terrain-shot transaction. The observer supplies visibility and shared
coordination, movement and lock costs; range and weapon expenditure remain owned
by the firer. An occupied coordinate continues through unit-target combat.

A firer's unit lock overrides the observer. Without a unit lock, its terrain mode
controls clearing/ignition behavior, while a firer with no coordinate lock can
still use the observer's coordinate. Indirect launches bypass the firer's weapon
arc and direct visibility checks, but retain weapon, crew and observer admission.

Tests cover all Mech/vehicle shooter-observer pairings behind blocked sightlines,
terrain impacts, native/Lua parity, no firer lock, observer loss and visibility
loss, rollback of inventory/terrain/dice, and database replay.


### Heading during Mech jumps

Bipeds and quads accept heading commands while jumping. The shared movement
heartbeat changes facing independently of the committed jump path, position and
landing destination. Ground and jump rotation share shortest-arc normalization
and overshoot handling.

Normal jump turning uses three degrees per current gravity-adjusted whole jump
MP each second, doubled for quads. FASA jump turning is eighteen degrees per
second. Surviving thrust is sampled each update. Uncontrolled descent retains its
heading-control restriction.

Tests cover both chassis and policies at multiple gravities, jet loss, wraparound,
no overshoot, native/Lua controls, callback rollback, database replay and identical
flight trajectories with and without facing changes.


### Ground vehicles on authored terrain

Building, wall and high-water map tiles now reach the shared vehicle elevation
and hazard rules. The blanket unsupported-terrain stop has been removed. Tracks,
wheels and hovercraft can traverse level and one-level transitions; steep faces
still use the existing avoidance and crash consequences. High-water falls retain
their water damage reduction.

Regression scenarios compare building/wall movement with equivalent grass height
profiles and cover high-water traversal, steep barriers, smoke, preserved material
on ordinary travel and database replay for each drivetrain.


### Shared indirect-fire experience

Mech and vehicle shooters now use the same pre-impact spotting and artillery
skill awards, including on misses. The common helper applies a newly earned
spotting level to the current shot. Vehicle reports retain accepted awards in
`experience_messages`, and the host publishes them in the firing transaction.

Tests rotate both shooter and observer chassis through character eligibility,
disconnection, cooldown and coordinate-lock exclusions. They also verify current-
shot level gains, callback rollback of balances and messages, and saved awards
and levels after restart.


### Signed mine strengths

All conventional mine types accept negative strength. Nonpositive material damage
applies no packets or location dice. Command/vibra strengths at most -2 still
visit neighboring hexes, emit blast notices and ignite eligible woods. Weak
fields are removed through the ordinary mine transaction.

Inferno heat remains signed: Mechs and stationary vehicles share one burn-timer
adjustment, with a minimum one-second timer for nonzero exposure. Negative heat
shortens an existing burn; mobile vehicles retain their configured fire/explosion
checks independent of heat magnitude. Blast `burn_seconds` reports the signed
adjustment; the stored timer remains nonnegative. Vehicle blast-heat APIs accept
signed heat. Missile inferno APIs remain positive-hit operations.

Tests cover bipeds, quads, tracked/wheeled/hover/stationary vehicles and landed
VTOLs; minimum signed strength, -2/-1/zero boundaries, existing burns, both fire
policies, neighbor ignition, deterministic replay and restart. Radio command-mine
tests include negative strengths through native/Lua dispatch and callback abort.
Late-failure regressions now inject unavailable woodland-ignition randomness
after earlier effects, preserving rollback coverage without rejecting valid mines.
This closes the negative-strength gap identified by the operator-inspection audit.


### Facing during forced descent and quad turning

Running Mechs and vehicles can request heading changes during forced descent,
subject to existing pilot, movement, fortification and hull-down restrictions.
One facing update uses the shared turn calculation and loaded movement ceiling;
descent cursors and translation retain their existing owners. Ground vehicle
descent validation permits a desired heading while still forbidding translation.

The reference heading command in `movement/mech_move_controls.c` and event
scheduling in `unit/mech_lifecycle.c` do not exclude falls. Its
`movement/mech_update_motion.c` applies the quad multiplier to ground turning as
well as non-FASA jumps. Rust now supplies that multiplier to the common turn
calculation; fixed FASA jump turning remains unchanged.

Regression coverage compares turning and non-turning falls for bipeds, quads,
tracked/wheeled/hover vehicles and VTOLs in both turn modes. It verifies rate,
wraparound, no overshoot, unchanged descent progression, native/Lua parity,
callback rollback and saved replay. Ground turns also verify the quad rate.

Falling VTOL heading events also call the ordinary fuel routine before turning.
Its low-speed draw, configured free-fusion policy and exhaustion handling remain
shared with powered flight. Tests include exhausted and already-announced empty
tanks and verify deterministic fuel draws alongside the independent descent.

The facing phase runs after ordinary motion and descent: aircraft that recover
lift have already used powered turning and are excluded from falling-unit facing.
A same-tick recovery regression verifies that the heading advances only once.


### Case-insensitive equipment identities

Weapon and system identities, ammunition namespaces, Artemis slot references,
split markers and TSM slot detection accept ASCII capitalization variants. The
shared equipment resolvers retain typed identities and preserve authored text
for inspection and persistence. Unknown names, malformed mounts and unsupported
modes still fail construction.

The reference `unit/mech_partnames.c` lowercases both registry keys and lookup
requests. Rust tests cover every weapon identity, mixed-case critical runs on
Mechs and vehicles, split mounts, unchanged loadout/mass and saved raw metadata.

A fresh audit of the 1,745 game assets finds 1,249 constructible assets among
1,302 biped/quad Mechs and 269 among 269 in-scope ground/VTOL vehicle assets.
Four Mechs now construct without asset edits: BloodKite-1, Hellion-A, Hellion-Prime
and PXH-IIC. No previously constructible asset regressed. The remaining 53 Mech
rejections include malformed or incomplete equipment and unsupported features;
these require individual classification. Constructibility does not prove full
simulation parity.


### Targeting-computer weapon eligibility

The shared eligibility query now admits HeavyFlamer, VehicleFlamer and
VehicleHeavyFlamer, matching their ammunition-fed category in the reference
weapon catalogue. Ordinary IS/Clan Flamers and machine guns remain excluded.
Artillery is excluded explicitly; zero missile count alone does not identify a
weapon that can use a targeting computer. All eight supported artillery identities
are covered by the eligibility regression.

The same query owns construction-mode admission, Mech and vehicle aiming, and
called-shot eligibility. Regressions cover the three flamer variants through
Mech native/Lua aiming and damage, vehicle attacks on both target chassis,
computer critical loss, read-only queries and saved replay.

Authored OnTC links are handled separately from automatic hardware eligibility;
see “Authored targeting-computer links” below for the completed integration.


### Repeated template comments

The shared Mech/vehicle parser accepts repeated case-insensitive `Comment` fields
and retains their text in source order, separated by newlines in
`attributes.comment`. Comments do not change the active section or create
equipment. Size and delimiter limits still apply, and duplicate ordinary unit
fields still fail. The reference loader's Comment case carries no game state.

Tests cover Mechs, ground vehicles and VTOLs, comments within sections, unchanged
loadouts, malformed input, size bounds, and Grendel-Prime inspection through
native and Lua commands. A fresh 1,745-file audit now constructs Grendel-Prime
without editing the asset and finds no construction regressions: 1,250 of 1,302
biped/quad Mech assets construct. The remaining 52 require further classification;
vehicle construction coverage remains 269 of 269 from the preceding audit.

### Case-insensitive ammunition rows and initial sizing

The shared source parser now recognizes the `Ammo_` prefix without regard to
ASCII case, including rows with whitespace-separated flags, numeric brands and
trailing dash placeholders. Mech construction uses the same prefix query when
normalizing initial bin quantities. Equipment spelling remains authored metadata;
live ammunition is still restored without construction-time refilling.

Regressions exercise source parsing and construction for Mechs, ground vehicles
and VTOLs, shared loadouts and mass, saved identity, and lowercase names across
all ammunition-using weapons and the existing quantity/half-ton boundary cases.

The rewrite's Daishi-H template uses whitespace-separated `Clan FlipArms`
Specials flags, as required by the Specials grammar. Daishi-H now constructs
with six weapons and four ammunition bins. The full asset audit finds no
construction regressions; 1,251 of 1,302 biped/quad Mech assets now construct.
The reference asset is unchanged.

### Authored template syntax corrections

Eight rewrite assets now construct after syntax-only corrections:

- ARC-6W and SGT-8R use pipe separators between weapon mode flags.
- Dra-A and PhoenixHawk-IIC2 include missing mode placeholders.
- Koshi-P, Ryoken-H and SD1-OD retain inactive equipment notes as flat Comment
  text, without nested opening braces. Koshi-P remains unarmed; its commented
  weapons have not been activated.
- Uller-D's left-arm ammunition occupies slots 8–9, preceding the equipment in
  slot 10, replacing the invalid descending range 8–0.

The complete asset audit finds all eight newly constructible and no regressions.
The current Mech count is 1,259 of 1,302 biped/quad templates; 43 remain rejected.
No reference-tree assets or Rust construction rules changed in this correction.

### Conflicting critical-slot corrections

Three rewrite templates now have non-overlapping equipment allocations:

- BlackHawk-H retains five two-slot heavy medium lasers per arm, occupying pairs
  3–4 through 11–12 after the shoulder and upper actuator.
- Supernova-1 retains three ER large lasers per arm in slots 5–7. Its four arm
  heat-sink slots move to unused positions 8–11, preserving installed equipment.
- BloodAsp-Prime removes the right-hand actuator row that conflicted with the
  heavy medium laser in slots 4–5, matching the handless left-arm layout.

The asset audit constructs these units with 10, 6 and 7 weapons respectively and
finds no regressions. Current construction coverage is 1,262 of 1,302 biped/quad
Mech templates, with 40 still rejected. Rust overlap and installation validation
remain unchanged, as does the reference tree.

### Mixed relocated engine installations

Complete Inner Sphere torso installations may retain seven center criticals and
mixed side counts after relocation. Ten-slot layouts with a one-slot side and
twelve-slot layouts with a one- or three-slot side now resolve as XL, including
mixed Light/XL side evidence. The reference's per-section technology discovery
sets XL for one or three slots; XL takes precedence in engine identity and the
implemented defensive BV calculation. Physical critical locations are retained.

AXM-3S, HBK-5S, MAD-4S, STK-8S and BTZ-3F now construct without asset changes.
Regression coverage checks mirrored layouts, mass, defensive battle value,
three-hit destruction, retained installation identity and saved damaged units.
Incomplete totals, inconsistent side evidence and non-torso engines remain
rejected. A complete asset audit finds these five newly constructible and no
regressions: 1,267 of 1,302 biped/quad Mech templates construct, leaving 35.

### Consistent launcher manufacturer metadata

Eight rewrite assets now use consistent manufacturer metadata across each
multi-slot weapon. BlackLanner-B, Lan-B and Nig-D retain LRM brand 5;
EXT-5E and KTO-21 retain iNarc brand 3; RFL-IIC_SJ retains UltraAC/2 brand 5.
Tur-P and Turkina-Prime use explicit zero throughout their unbranded LRM mounts,
replacing the mixture of zero and omitted brand fields. Weapon identities,
counts, modes and critical locations are unchanged.

The complete construction audit finds all eight newly constructible with no
regressions. Coverage is now 1,275 of 1,302 biped/quad Mech assets; 27 remain
rejected. Rust mount consistency checks and the reference tree are unchanged.

### Arrow launcher metadata and ammunition flags

ANV-8M now uses manufacturer 3 across its entire Clan Arrow IV launcher.
Matador-1's machine-gun ammunition uses canonical `Hotload`; VKG-2F uses
canonical `Halfton`. Equipment identities, critical allocations and ammunition
quantities are unchanged. The full asset audit finds all three newly
constructible and no regressions: 1,278 of 1,302 biped/quad Mech assets construct,
leaving 24. The reference tree remains unchanged.

### Split Arrow IV mounts

The shared Mech mount resolver admits IS Arrow IV installations with explicit
split links. The extension count determines the primary run length, so a linked
mount need not fill the primary section to slot 12. Complete contiguous runs,
matching metadata, adjacency, valid parent pointers and exact total slot counts
remain required. Split eligibility is defined once on the weapon identity and
used by both link validation and mount construction.

CPLT-C5 retains its nine right-arm and six right-torso Arrow IV slots. Its torso
entries now explicitly link to the arm mount; the loadout contains one fifteen-slot
launcher. Regression coverage verifies damage in every primary and extension
slot, incomplete-link rejection and saved damaged-state round trips, alongside
existing split gun, section-loss and explosion tests.

The complete asset audit finds CPLT-C5 newly constructible with no regressions:
1,279 of 1,302 biped/quad Mech assets now construct, leaving 23 rejected.

### GOL-3S torso engine and split cannon

GOL-3S now keeps both left-side light-engine criticals in its torso. The LBX-20
occupies torso slots 3–12 plus an explicit extension in rear-left-leg slot 5,
replacing the engine slot previously placed there. The eleven-slot cannon and
all other equipment remain installed. This uses existing split-mount handling;
no Goliath-specific runtime rule was introduced.

The engine regression checks the corrected Light family and eleven-slot mount,
and still rejects an engine inserted into a leg. Engine and split-weapon tests
pass. The full asset audit finds GOL-3S newly constructible and no regressions:
1,280 of 1,302 biped/quad Mech assets construct, with 22 remaining rejections.
The reference tree is unchanged.

### Complete heat-sink groups in authored assets

Nine rewrite templates now allocate complete heat-sink groups without changing
their declared cooling capacity or installed weapon identities:

- HMR-3P completes its right-arm three-slot sink in slots 6–8.
- Nig-B and NightGyr-B complete their left-arm sink in slots 9–10.
- RFL-3NC completes its left-torso sink in slots 1–2.
- Clint-IIC completes its right-torso sink in slots 3–4 and moves the jump jet
  from slot 4 to previously unused slot 12.
- Naga-A, Naga-B, Naga-C and Naga-Prime move each incomplete torso sink to a
  complete pair in previously unused slots 6–7.

The complete asset audit finds all nine newly constructible and no regressions:
1,289 of 1,302 biped/quad Mech assets construct, leaving 13 rejections. Runtime
sink grouping remains strict, and the reference tree remains unchanged.

### Remaining conventional system corrections

Five rewrite assets now construct after completing their equipment definitions:

- AS7-DC uses supported Clan ER medium lasers for its four nonexistent
  `CL.MediumLaser` entries, retaining the two rear-facing mounts.
- WVR-6MCL installs its missing second life-support critical in free head slot 4.
- Piranha moves its incomplete head sink to a complete right-torso pair, 11–12.
- Piranha-1 completes both leg sink pairs in 5–6 and moves its incomplete center
  sink to left-arm slots 6–7.
- Masakari-C uses two left-arm sink pairs in 3–6 and completes its remaining
  partial sink in free right-arm slots 11–12.

Declared cooling fields remain unchanged. The complete asset audit finds these
five newly constructible and no regressions: 1,294 of 1,302 biped/quad Mech assets
construct, leaving eight rejections. No runtime construction checks were relaxed,
and no reference-tree files were modified.

### Authored targeting-computer links

Weapon mounts retain explicit `OnTC` independently of automatic catalogue
eligibility. One shared aiming query serves Mechs and vehicles: an explicit
link enables assistance, or an eligible weapon gains assistance from installed
computer slots; any unavailable computer slot disables it, and cluster ammunition
never benefits. Explicit links can exist without physical computer slots, matching
retained authored links in the reference loader. Computer installation/status
queries still require actual equipment. Automatic machine-gun eligibility remains
false; authored links do not alter the catalogue.

Ammunition `OnTC` is retained in template metadata but does not link a weapon or
change its ammunition mode. Goshawk-1, Goshawk-2, Thor-D and Viper-2 now construct
with their authored weapon links. Mas-A also constructs after its separate LRM
manufacturer mismatch is corrected to brand 5 across the mount.

Regression coverage checks automatic versus explicit links, absent and damaged
hardware, cluster exclusion, vehicle attacks on Mechs and vehicles, damage loss,
read-only aiming and saved replay, plus preserved authored Mech links and inert
ammunition flags. The full asset audit finds five newly constructible and no
regressions: 1,299 of 1,302 biped/quad templates construct. The three remaining
rejections contain LAM conversion equipment, whose flight behavior remains
outside the current aerospace scope. Other gameplay parity still requires audit.

### Persistent TIC membership

Mechs and vehicles share four ordered weapon groups numbered 0–3. `addtic`,
`deltic`, `cleartic` and `listtic` use the same membership operations as
`btech.unit.tic` and `btech.unit.tic_edit`. Native selections support numbers,
inclusive ranges and comma-separated lists; `deltic <group>` clears that group.
All changes require the conscious assigned pilot physically in the cockpit.

Selection validation precedes mutation. Weapon damage does not renumber or remove
members; group state persists with the owned unit and is validated against the
installed weapon count on loading. Tests cover Mechs, ground vehicles and VTOLs,
duplicate additions, invalid edits, wrong-pilot access, callback rollback,
critical loss, saved replay and corrupt group members. Group firing remains
unfinished and is the next acceptance item in the delivery audit.


### Ordered TIC firing

`firetic <groups> [#unit]` and `btech.unit.tic_fire(unit, pilot, groups, target?)`
use the ordinary configured firing action for every selected weapon, including
launcher failures, expenditure, damage, crew consequences and observer messages.
Groups and weapon numbers run in ascending order; overlapping groups retain
repeated attempts and ordinary recycling checks. A rejected shot restores that
attempt and continues. Recoil falls and shutdown stop the batch. Publication
failures and failed Lua callbacks restore the entire batch and its messages.
Empty groups click only after startup, map and cockpit authority checks.
`listtic` now uses the common weapon readiness display, including damaged mounts.

Cross-chassis tests cover bipeds, quads, tracked vehicles and VTOLs, compare grouped
results with ordinary firing, and exercise duplicate selection, failed shots,
callback rollback, native/Lua agreement and saved replay. The Heavy Gauss recoil
scenario verifies firing stops before the next group. Target arguments currently
follow the ordinary Rust `fire` interface (explicit object or saved lock); remaining
reference target grammar belongs to the cockpit interface audit.

Validation: the complete suite passes 1,865 tests; all-target Clippy with warnings
denied, formatting and diff checks pass. The reference tree remains unchanged.


### Heat cutoff regulation

Mechs persist one heat-cutoff state containing its setting, intentionally disabled
cooling points and a four-second toggle countdown. `heatcutoff` and
`btech.unit.heatcutoff(unit, pilot)` share conscious assigned-pilot admission and
the `battletech.heatcutoff` gate. Admitted countdowns continue with the reactor off
and survive restart; failed Lua callbacks restore the pending toggle and notices.
`btech.unit.state(unit).heat_cutoff` and cockpit status expose the state.

The existing thermal sample applies water, inferno and temperature effects before
regulating stored weapon heat plus continuous production minus dissipation.
The regulator changes at most two points per sample (four for double/Clan sinks),
using the 9–10 band and restoring cooling gradually after disengagement. Current
sample water bonuses remain until recomputed on the next sample. Physical damage
remains separate; disabled capacity clamps to surviving cooling at the next
sample. Idle heartbeat admission includes pending transitions and restoration.
Vehicles retain their existing thermal model; this control belongs to Mechs.

Validation: the complete suite passes 1,869 tests. After matching the reference
command’s ignored trailing text, all three heat-cutoff integration tests pass
again. Final all-target Clippy with warnings denied, formatting and diff checks
pass. The reference tree remains unchanged.


### Preferred ammunition sections

`usebin <weapon> <section|->` and `btech.unit.usebin(unit, pilot, weapon, section?)`
share conscious assigned-pilot, map, weapon and section admission. Reactor power
and present matching ammunition are not prerequisites. Biped/quad and vehicle
anatomies supply typed persisted preferences to one feed-priority rule: preferred
section, mount section, then canonical section/slot order. Ordinary launcher,
burst fallback, misload and unjam consumers continue using the shared feed planner.
A preference never bypasses mode, supply or critical-availability checks.
Automatic missile defense intentionally ignores the preference, matching its
separate reference lookup request. Energy weapons, including catalogue Laser AMS
entries that retain ammunition fields, cannot select a preferred section.

Weapon inspection exposes `preferred_ammunition_section` and the common cockpit
weapon display shows it. Tests cover native/Lua agreement, failed callback rollback,
restart, actual expenditure, recycling admission, damaged/empty preferred bins,
and normal versus special ammunition fallback across supported chassis.

Validation: the full suite passes 1,872 tests. After matching ignored trailing
arguments, all five ammunition-feed/preference tests pass again. Final all-target
Clippy with warnings denied, formatting and diff checks pass. The reference tree
remains unchanged. The delivery audit records a separately discovered idle heat
cutoff discrepancy that still needs correction.


### Independent cutoff timing and automatic turret tracking

Idle stopped Mechs now finish heat-cutoff toggles without regulating cooling or
advancing thermal-check clocks. Both suppression and gradual restoration wait
until thermal accounting resumes. The lifecycle test covers both toggle
orientations and subsequent startup; the idle-server test verifies the saved
transition finishes without a map or reactor power.

`autoturret` and `btech.unit.autoturret(unit, pilot)` persist one mode on vehicle
state. Shared tracking reads either target anatomy through the scanner projection
or the selected hex, updates hull-relative facing once per committed second, and
does not wait for lock settling. Power, consciousness, surviving turret,
jam and lock conditions govern updates. Invalid, deleted and cross-map unit
references do not rotate the turret. Tracking precedes scanner refresh and shares
the server's rollback boundary. The integer bearing preserves signed tenth-degree
quantization and the 180-degree same-point convention.

Tests cover controls and callback rollback, live target movement, hex selection,
hull-relative facing, shutdown/jam/lock gates, saved replay and a running server.
Ground propulsion types and turret-equipped rotorcraft use the same implementation;
rotorcraft without turrets reject the control. Sensor-flash blindness remains an
explicit acceptance gap, recorded in `btech-delivery.md`.

Validation: the full suite passes 1,876 tests. All thirteen cutoff/turret tests
pass after the final turning refinements: crew stun permits rotation, unconsciousness
pauses it, and fractional hull turns preserve the tracked world bearing. Final
all-target Clippy with warnings denied, formatting and diff checks pass. The
reference tree remains unchanged. Reactor explosions and their temporary
sensor-flash effects remain acceptance work in the delivery audit.

### Reactor blasts and temporary sensor blindness

A trusted callback can call `btech.unit.reactor_explode(unit)` to detonate a
constructed Mech. The action shares blast packets, heat exposure, neighboring
forest ignition and casualty publication with existing combat. It checkpoints
all world and notification changes, including failures in departure callbacks.

Visible observers with infrared or light amplification as their primary sensor
are blinded for four committed seconds. Saved recovery runs while stopped and
suppresses cockpit controls/readouts, visual broadcasts, spotting and automatic
turret tracking. Trusted unit state exposes `blinded_remaining`. Tests cover
cross-chassis blasts, sensor susceptibility, forest ignition, replay, casualties,
rollback, persistence and live-server recovery.

Engine-instability triggers, cockpit countdowns and terminal crew injuries are
implemented as described below. Ejected MechWarrior infantry remains within the deferred infantry
scope; the implemented fatal-crew path uses existing evacuation behavior.

Reactor milestone validation: all 1,880 tests pass, along with formatting,
Clippy across all targets with warnings denied, and diff checks. The reference
tree remains unchanged. The broader acceptance work is tracked in
`btech-delivery.md`.


### Cockpit self-destruction and terminal crew injuries

Native `explode` and trusted `btech.unit.explode(unit, pilot, text)` use one
controller and durable scheduler for Mechs and vehicles. `unit.state` exposes
`self_destruct` and `self_destruct_safe`; scenario callbacks can set the latter
through `btech.unit.explode_safe(unit, safe)`. The safety flag gates new ammo
requests. Configuration, cooldown, wizard override and in-character pilot
ownership checks follow the cockpit behavior; engagement releases the pilot.
Optional `MechDebugInfo` announcements share transactional publication.

Countdown state retains scheduling order and the reference's actual modulo-256
mode interpretation, including zero and negative configured delays. Mechs reuse
the existing reactor or ammunition explosion actions; vehicles destroy the rear
section and receive the final crew injury. Wreck placement first selects terrain
support, then adds six levels. Descent is scheduled according to the original
altitude, including the distinction between bridge decks and beds.

Terminal injury reuses tactical consciousness and recovery after pilot release,
preserving previous unit injury counts without adding personal-health damage to
an evacuated character. New wreck recovery survives restart. Self-destruct tests
cover seven chassis, native/Lua controls, admission exceptions, countdown order,
live ammunition cancellation, restart, callback rollback, failed server commits,
and elevated wreck placement across all supported vehicle classes.


### Reactor instability from section loss

Section destruction and flooding apply the configured stackpole check when
engine losses reach three. Ordinary engine criticals do not perform this check. It reuses the reactor blast resolver and retains nested reports
for atomic casualty publication, including ammunition and other damage cascades.
Eligibility includes 30 elapsed seconds after the first pristine center-torso
internal hit; that hit opens the window after its criticals. Initial world
startup retains the reference's zero-timestamp grace. A successful 2d6 roll of
nine or more requires a running or starting reactor; an eligible stopped reactor
still consumes the roll. Supercharger overload follows its separate failure rule.

Both bounded clocks expire on committed server ticks, persist across restart,
and roll back together with damage and notifications. Trusted Lua unit state
exposes `reactor_instability_remaining`; nil uses the initial world grace and
zero means the damage window expired. The `stackpole` and `explode_reactor`
settings supply runtime admission and hit-table policy.


### Manual reactor reentry and neighboring chains

Destroying a reactor manually uses the same engine-compartment instability check
as combat damage. When eligible, the nested explosion finishes before the
original blast. The report's optional `section_explosion` contains that event;
its notices and casualty effects share the caller's checkpoint. Recorded engine
losses bound reentry, while a fresh request after head destruction is rejected.
Tests cover both blast passes, terminal crew injuries, timing, power and replay.

A reactor can also trigger a neighboring reactor through ordinary blast damage.
The second report remains inside its triggering impact, and its blast can reach
the original wreck without retriggering it. Cross-unit damage, casualty callback
rollback and save/reload tests cover the complete chain. The reference's later
cleanup of fully destroyed in-character objects remains acceptance work.

### Mech FASA and critical-proof hit routing

Mech weapon-location resolution consumes its initial routing roll before any
configured delegation. With `fasacrit` enabled, FASA draws a second 2d6 location
roll and makes its roll of two a through-armor critical. `CritProof_Tech` is now
accepted for biped and quad construction; it takes precedence over FASA, uses a
second location roll, and suppresses component critical selection. Both routes
reuse the ordinary weapon-location rows and the anatomy-specific punch rows for
configured head grazes. A combat-safe target still consumes the delegated roll
but returns before graze effects or their additional die.

Critical-proof construction is distinct from combat safety: armor and internal
structure still take damage, head hits still cause injury, and an internal roll
of twelve can still destroy a limb or head. Armor/internal critical-count rolls
are retained even when component selection is suppressed. Physical and partial
cover location tables do not acquire an extra weapon-routing roll.

Focused validation covers both anatomies, all attack arcs and 2d6 totals,
configuration/technology precedence, head-graze modes, immune targets, rejected
inputs, critical-count sequencing, native/Lua firing, callback rollback and
restart replay. The delivery audit records the full-suite result and remaining
integration work.


### Swarm and Swarm-1 ammunition

`fireswarm` and `fireswarm1` use the shared cockpit weapon selector on Mechs,
ground vehicles and VTOLs. Lua exposes matching `btech.unit` operations. Compatible
indirect launchers accept typed `Swarm` or `Swarm1` weapon/bin flags; rockets and
dead-fire missiles are excluded. Live mode selection, bin quantities and random
streams survive saving without reinitialization.

One shared flight resolver begins after an initial hit and carries unused missiles
through nearby targets in map order. Swarm-1 excludes friendly secondary targets;
ordinary Swarm can return to its own launcher. Secondary selection requires a
retained visible contact with unblocked terrain, or clairvoyance, and distance
strictly below 1.9. It skips already attacked and combat-safe units. Misses on
secondary targets retain the missiles. Cumulative range is checked before each
attack, and ten previously visited slots permit eleven attacks in total. Both
ammunition types bypass AMS. Empty-coordinate shots use ordinary terrain effects.

Each hit reuses the existing Mech or vehicle salvo resolver, capped by the number
of incoming missiles. Original launcher attribution, adjusted hit threshold and
glancing state persist through the flight. Reports expose ordered `hops` under
`salvo.kind = "swarm"`; each hit contains its ordinary anatomy-specific salvo.
Publication and callback rollback include every hop in the launch transaction.


### Shared cluster, smoke and mine controls

`firecluster` aliases `cluster` and now uses shared ammunition admission and
storage for all supported chassis, including vehicle artillery. It preserves
existing Smoke/Mine artillery selections by rejecting a conflicting change.
Lua exposes both names through the same implementation.

`firesmoke` and `firemine` follow the reference selector's actual eligibility:
missile launchers, excluding rockets and integral one-shot weapons. The reference
passes special-kind 4, and compares the weapon type to the IDF/DAR bit mask;
with TMISSILE=1, IDF=0x40 and DAR=0x80 that expression admits all missile types,
including indirect and dead-fire launchers, and rejects artillery. Literal Smoke
and Mine bin flags are therefore supported for missiles as well as artillery;
the combined Artemis/Mine and Narc/Smoke spellings still mean guidance modes.

Missile Smoke/Mine rounds retain conventional damage. Mine rounds bypass AMS;
Smoke rounds use ordinary interception. They do not deposit environmental
payloads. Authored artillery Smoke/Mine supplies retain their delayed smoke/mine
arrival effects. Shared native/Lua controls, selection persistence, real cluster
launch/arrival replay, missile damage/interception and control failures are covered
across supported chassis. The original reference tree is unchanged.

### Weapon critical degradation

Ordinary Mech weapon criticals can damage a component while leaving the mount
usable. Damaged slots are retained in `weapon_damage` and excluded from future
random critical selection. Every additional slot raises the damage-table roll;
a hit beyond half the mount's slots forces destruction. Explicit equipment-loss
operations still destroy the selected slot. Explosive weapon-critical handling
precedes ordinary degradation.

Moderate damage affects accuracy at all ranges. Focus and ranging damage add an
accuracy penalty outside short range; focus damage also reduces energy damage
before glancing rounding. Crystal damage adds heat and an explosion risk. Barrel
damage can cause a permanent loader jam. Feed damage can cause an internal
explosion and prevents changing firing or ammunition modes. Failure checks reuse
the attack dice. A permanent critical loader jam is distinct from a manual feed
jam and cannot be cleared by the ordinary unjam control.

`BattleUnit::weapon_damage()` and Lua `btech.unit.state(id).weapon_damage` expose
the damaged slots. `weapon_damage_effects(index)` derives a mount's penalties
across all of its slots, including split sections. Launch reports carry the
resulting energy damage penalty into the shared Mech, vehicle and terrain packet
resolver. The vehicle launch adapter supplies zero degradation penalties; no
second vehicle implementation or new repair subsystem is introduced.


### Orbital insertion and cocoon combat

Wizard `@ood x y [z]` and Lua `btech.unit.ood(actor, unit, x, y, z)` insert a unit
on its current map, with a default elevation of 300. Ground chassis descend under
mass-based cocoon protection. VTOLs enter ordinary flight state; an unpowered VTOL
waits at the inserted altitude until startup completes. Placement and all output
roll back together if the enclosing action fails.

Intact cocoons give attackers a -2 aim modifier. After ordinary damage-entry
checks, a roll above eight diverts the entire material packet into the cocoon,
including excess damage beyond its remaining integrity. Armor, internal structure
and criticals remain untouched by that intercepted packet. Armor overflow does
not get another interception roll. Internal explosions use the same protection.

Firing opens the cocoon after mechanical failure checks, including when a Streak
launcher fails to lock. Surviving jump jets compensate; otherwise the unit enters
the existing forced-descent clock. Unit, coordinate and artillery fire share this
transition, and reports expose `launch_notices`. Shared aim reports expose the
`orbital_drop` modifier, which becomes zero after a breach. Descent and protection
survive restart. An attached `on_ood_land` callback runs once on touchdown, after
the touchdown notice and before landing dice, XP and damage. The event receives
`operation = "ood_land"`, with object, enactor and cause all identifying the unit,
empty arguments and no source/destination. Changes to live placement and combat-safe
state are used by the landing continuation; removal or cancellation prevents that
continuation. Callback errors restore the entire airborne tick, including other
units that already landed and all staged feedback.


### Named unit fields

Wizard @VIEWMECH and Lua `btech.unit.fields(actor, unit, arguments)` use one field
catalogue and the map/station column renderer. Fields retain their complete names
in detached reports; optional `1`/`4` layouts and case-insensitive prefixes only
change presentation. Initial projections cover identity, crew, team, XP scaling,
motion, retained placement, mass, towing and orbital protection. Continuous field
coordinates use 322.5 units per horizontal hex and 64.5 per elevation level;
normalized Rust geometry remains unchanged. Unmapped fields report n/a, not zero.

@SETMECH and `btech.unit.set_field(actor, unit, field, value)` currently support
team and xpmod. Team uses the existing signature service, including command-network
invalidation. XP scaling uses the existing finite/nonnegative validation, retaining
other experience settings. Setters are silent, wizard-only and transactional, and
can administer detached units. Other unit-field setters remain unfinished.

@SETSPECIAL/@VIEWSPECIAL choose the current map, station or physical unit and call
those same field services. They introduce no separate mutation or formatting rules.

Named unit inspection now projects `lrsrange`, `scanrange`,
`tacrange`, `radiorange`, and `radiotype` through the existing scanner and radio
services. VTOL `fuel` and `fuel_orig` expose saved remaining fuel and original
tank capacity. The named `fuel` setter delegates to the shared VTOL refill
service, including capacity validation and load reconciliation. Other unfinished
unit-field readers and setters remain outstanding.

Named `mechname` and `mechref` edits now update saved unit identity for every
supported chassis through common validation. They preserve the existing combat
state and synchronize the template attributes with the resolved identity. Native
and Lua entry points share rollback and publication rules. Runtime hardware setters now preserve explicit zero values independently of
template defaults, including restart and later sensor critical damage. Sensor
ranges accept 0–127, radio range 0–32767, and radio configuration 0–255.
The `targcomp` field now reads and selects modes 0–4. Shared conventional aim
applies range bias, multiple-target tracking and anti-air modifiers independently
of installed targeting-computer equipment. The reported `targeting_mode` term
is included in unit and coordinate shot subtotals.

Anti-air tracking now uses shared orbital-drop ownership for both Mechs and
ground vehicles. Its two-point tracking bonus (three with AntiAircraft
equipment) stacks with the separate cocoon target modifier. Multiple-target
side-arc penalties remain independent of the settled lock and weapon-arc
override setting.

Unit field inspection now exposes `heat`, `dissheat`, `overheat`, `disabled_hs`,
and `heatsinks`. Mech production and dissipation retain the committed heartbeat
sample, including stored weapon heat before cooling; inspection does not sample
or consume state. Saved samples survive restart. Surviving cooling capacity is
shared with compact status export for both anatomical models. Mech thermal setters cover `heat`, `dissheat` and `overheat`; `disabled_hs` is
read-only. They edit samples or excess without changing stored weapon
heat or running hazards. The next thermal sample resumes normal computation.
Construction edits and the remaining unit-field contract are still outstanding.

Unit fields `centdist` and `centbearing` share the cockpit navigation geometry
without inheriting pilot, power or scanner admission. `sensors` formats the
shared active and pending selection state; sensor switching remains owned by
the existing heartbeat. These projections are read-only and preserve state and
pending transitions across inspection and restart.

Unit inspection now exposes authored `cargospace` through the shared load
admission lookup. `C3iNetworkSize` uses eligible C3i membership, excluding the
inspected unit itself. Both fields are projections; they neither alter cargo
accounting nor maintain a second network membership list.

The `jumpheading` and `jumplength` fields now read durable launch history,
captured only after route admission. History survives landing, failed subsequent
launches and restart. The launch record preserves field units independently of
normalized flight geometry; it does not drive movement or targeting.

Saved template metadata `unit_era` and `unit_tro` now participates in shared
identity inspection and editing for Mechs, ground vehicles and VTOLs. Both
construction paths enforce the same 24-byte bound. Missing metadata displays
`Undefined`; explicit values, including empty strings, survive persistence.
The remaining construction, status/critical and damage fields are outstanding.

Unit inspection now derives `numseen` from acquired enemy contacts through the
shared contact and team projections. Identification is not required, friendly
contacts are excluded, and team changes are reflected without maintaining a
second mutable counter. Inspection leaves detection state and dice untouched.

Parent cockpit notices now include registered live gunner stations through one
shared publication path. Firing uses this route instead of adding a separate
station copy. Private player messages do not expand to station audiences, and
a publication failure restores the pre-message output checkpoint. Explicit `turret0`–`turret2` fields now retain three deferred notice
destinations for each supported unit. The audience unions them with registered
stations, filters unavailable locations and deduplicates before publication.
Station parentage remains independent; `unusablearcs` is still outstanding.


Display-name configuration now shares one validated saved value across Mechs,
ground vehicles and VTOLs. The wizard `displayname` field and Lua
`btech.unit.set_display_name` update that value without changing construction
identity, equipment, assigned IDs or random state. Empty strings clear the
override; nonempty values have a 120-byte limit enforced on edits and restored
records. `btech.unit.display_name` reads the raw override. Cockpit status and the
shared identified-contact projection resolve the template-name fallback, while
unidentified contacts remain concealed. Field edits and Lua callbacks retain
world and publication rollback.


Startup history now records the supplied Unix time at completion through one
shared power transition for Mechs and vehicles. The mechanical
`advance_battle_units(world, now)` API requires the timestamp explicitly; server
updates pass the existing injected schedule clock, keeping deterministic replay
independent of wall-clock reads inside the domain. `last_startup` begins at zero,
survives shutdown and aborted starts, and is replaced only when startup completes.
It persists on every supported chassis and is exposed through unit-state Lua and
the named-field reader/setter. Administrative edits accept signed 64-bit values
without changing power state. Failed saves restore history with the rest of the
tick, and Lua callback failure restores edits and output together.


Vehicle notice preferences now use the shared SLWarn and AutoconShutdown metadata,
setters and delivery services. Vehicles retain an illumination observation cursor
alongside the two preference values, defaulting to false. Both construction stores
feed one pending-work check and one illumination transition publisher; saved
observations prevent repeated alerts after restart even when a warning is disabled.
The shared scanner projects vehicle shutdown-contact policy instead of suppressing
those notices unconditionally. Loss-of-lock warnings and contact acquisition remain
independent. Native preferences and existing Lua setters expose both controls, and
vehicle Lua state includes their current values. This covers receiving illumination
notices; vehicle searchlight hardware and controls require a separate integration.


Vehicle searchlight hardware now uses the same saved BattleSearchlight state,
switch admission/countdown, beam projection and damage-roll service as Mechs.
Vehicle snapshots validate installed equipment and lamp-state invariants. All
supported emitters feed the unit and terrain lighting queries, retaining their
separate 30-hex unit and under-60-hex terrain ranges and existing LOS policies.
Nearby burning vehicles also contribute inferno illumination through that shared
unit query. SLITE and Lua switching retain five-second timing across restart;
status and clear contacts display active vehicle lamps. Shutdown cuts lamp power,
and pending switches on shutdown units expire without reactivation. The scheduler
includes vehicle switch countdowns.

Ground-vehicle front armor hits expose an installed searchlight after combat-safe,
lost-section and cocoon admission. The same common rolls and notices serve front
torso Mech hits. Destroyed lamps turn off and cancel switching. Rear/side vehicle
hits and VTOL hits do not use this ground-only exposure rule. Damage and notices
remain within the existing transaction and restart boundaries.


Vehicle armor and ammunition warnings now share the Mech warning services.
Armor severity uses the same integer thresholds and transition-only policy, with
vehicle section abbreviations supplied to the shared message formatter. The armor
stage records its before/after severity and publishes after through-armor criticals,
before penetrating damage. Combat-safe, zero-damage, missing-section and cocoon
interception paths do not invent warning transitions.

Ammunition warning accounting now accepts anatomy-independent installed bins,
retaining half-ton/special-ammunition weights and single/double/gatling windows.
Vehicle launches inspect pre-expenditure state, suppress warnings for failed
launches, and carry an explicit ammunition_warning through direct, hex and
artillery reports. Warning preferences default enabled and persist on vehicles;
native and Lua controls use shared admission, metadata and state. Suppression
changes publication only, with existing transaction rollback preserving dice and
material state.


Named MechPrefs inspection now projects authoritative typed preferences instead
of reporting n/a. Both chassis families expose searchlight warnings, cliff-fall
policy, inverted armor/ammunition-warning suppression, shutdown-contact notices
and friendly-fire safety. Mechs also project tight-turn mode. Administrative edits
update those same saved booleans, preserving gameplay consumers and cockpit
queries. No separate raw preference mask is stored.

Map and unit fields share one bounded signed-32-bit/letter bitvector parser and
formatter. Preference writes validate the entire supported mask before mutation,
then participate in the existing wizard action's world/output rollback. MechPrefs
bit a represents disabled MechWarrior safety. Broader integration remains in progress.


Vehicle tight-turn mode is now connected to the common turnmode action, with
pilot ownership, Maneuvering Ace admission and callback/output rollback preserved.
Ground driving and VTOL flight control pass the saved setting into the same
throttle adjustment used by Mechs. The shared adjustment retains slowdown modes
zero/one, mode-two angular bands, and the additional 0.4-MP reduction only while a
heading change leaves at least one degree outstanding. Acceleration limits still
apply afterward. Vehicle state, status and MechPrefs bit i now expose this saved
preference; stationary platforms remain immobile regardless of its value.


BTHDebug and StandAnyway now have one shared typed preference store for every
supported chassis. Reference inspection found their configuration fields but no
active combat/standing consumers, so Rust retains their observable configuration
behavior without inventing attack modifiers, extra diagnostics or standing bypasses.
The BTHDebug cockpit/Lua toggle uses shared ownership checks; MechPrefs bits f and
j project and edit the saved booleans. Lua unit state exposes BTHDebug. Persistence,
invalid-actor rejection and callback rollback share existing boundaries, and
acceptance verifies that enabled flags do not change firing state or messages.


MechWarrior safety shares auxiliary preference storage across supported chassis.
The safety command, mechprefs MWSafety and Lua unit.mw_safety use the same pilot
checks and saved state. Status warns when disabled. Completed startup enables
safety while preserving other preferences; shutdown and aborted startup retain
the current setting. The reference firing restriction concerns MechWarrior unit
targets, which remain outside the admitted combat target types.

Manual SNIPE integration requires motion prediction without a second movement
implementation. Mech ground movement now calls a read-only proposal service for
turning, terrain/heat slowdown, lateral travel, load/equipment ceilings and
acceleration. It returns controls and an endpoint; committed traversal still owns
collisions, hazards, damage reconciliation and notifications. Vehicle motion
already separates proposal from traversal. The SNIPE command and prediction's
terrain-stop policy remain pending; this extraction does not admit autopilots.


Manual SNIPE now composes bounded horizontal prediction, the ordinary hex-target
setter and configured firing transactions. Native and Lua entry points require a
wizard assigned to the shooter. Per-weapon rejections retain ordinary launch
semantics; host/output failure rolls back the batch. Prediction and queued shells
share flight-time arithmetic. Mech and loaded vehicle proposals are also the live
movement implementations, avoiding an independent AI movement model. Prediction
uses fixed current orders/damage and a terrain stop policy, with no dice or world
mutation. It freezes at the last valid in-map point on exit, admits no map transfer,
and does not forecast future combat or vertical movement. No autopilot was added.


The named bv field now calls the same live Battle Value service used by gameplay,
with the host's TSM towing configuration. Native inspection and Lua unit.fields
format the total to two decimal places. No cached or independently writable unit
value is introduced. Damage and runtime weapon-value edits are covered across all seven supported
chassis fixtures. Reload retains damage and resets weapon overrides, so inspection
recomputes the value using catalogue defaults. Damage encoding and the remaining
status/critical and construction field contracts still require implementation.


### Wizard character catalogs

`+show` lists its accepted categories. `+show values`, `+show skills`,
`+show advantages`, `+show attributes`, and `+show allvalues` display the
reference three-column character catalogs and totals. Category names are
case-insensitive and must be complete. The reference help advertises `char_`
prefixes, but its lookup rejects them; this command preserves that behavior.
An argument after `=` is ignored.

`+show btechvalues` displays the reference special-field name inventory with
numeric object kinds (0 unit, 2 map, 4 gunner station), in reference order.
This inventory includes fields whose read/write behavior is still tracked in
the field audits; listing a name does not enable its setter. The report ignores
an `=scode` argument, as the reference does. All catalogs are wizard-only and
read-only, with no character initialization or experience changes.


`+charclear <player>` is wizard-only and accepts a player name, account alias or
`#dbref`. It clears personal skill/advantage values, XP and last-use timestamps,
sets all five attributes to 1, and sets bruise/lethal damage to 0. The explicit
Rust profile represents the reference's defaults when stored stats are absent.
Unit pilot assignments, cockpit injury counts and wreck state are unchanged.
An existing unconsciousness countdown and its dice stream continue; subsequent
checks use cleared health and advantages. Clearing does not immediately wake a
pilot or seed new recovery dice. Repeating the command is harmless.


### Startup pilot health

After startup admission, every supported cockpit initializes its injury count
from the operator's `(bruise + lethal) / (2 * Build)`, truncating the quotient.
A doubled Build outside 1–100 uses divisor 10. Counts saturate at 127; a missing
profile supplies zero health. This does not alter personal health, consume dice,
reset recovery timing, or declare a casualty. A high initial count is distinct
from confirmed tactical death. Subsequent tactical injury kills at six or more;
in-character injury follows personal health instead. Normal and override startup
use the same calculation, as do native and Lua calls.

The world stores confirmed tactical pilot death separately from the numeric
count. Physical crew-death cleanup still uses the existing chassis services.
The startup matrix covers all seven chassis in both character modes, a live
six-injury startup, subsequent injury, rollback and restart. The broader baseline finished in `target/audit-startup-health-full.log`; its
six failures were corrected and the affected suites now pass. Subsequent
movement/crew/field checks also pass. See the porting-audit progress log for
the exact scope of each run.


### Recovery feedback

An assigned pilot's recovery attempt privately reports
`You attempt to regain consciousness!` and
`Regain Consciousness on: <target>  \tRoll: <roll>` using the already-consumed
check. Success then sends `The pilot regains consciousness!` to cockpit occupants
and linked stations. A failed attempt exposes no roll to passengers and emits no
public failure message. Blinded cockpits suppress all three messages while the
recovery check and timer proceed normally. Empty crew recovery announces success
only. Player-owned recovery continues after cockpit release, with private
attempt/roll and outcome messages. These messages share the heartbeat's commit
and rollback boundary; rendering performs no additional roll.


Pilot damage changes share one count writer across Mechs and vehicles. It keeps
`pilotdam` and character-pilot reports aligned after startup, character injuries,
tactical injuries and administrative edits. Personal health remains separate;
changing the cockpit scalar does not heal the player. Counts above 127 are
rejected when loading unit state. A count of six alone does not mark a wreck;
confirmed tactical death has its own persisted marker.
