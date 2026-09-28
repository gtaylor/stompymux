+++
title = "@btech"
description = "Manage BattleTech assets, maps, runtime settings and inventory"
keywords = ["snipe", "@setmech", "@viewmech", "@setspecial", "@viewspecial", "@ood", "setmapindx", "setxy", "@weight", "@damage", "@damagesection", "@losemit", "setteam", "eventstats", "memstats", "listforms", "savedb", "xptop", "setxplevel", "@viewmap", "fixmap", "@setmap", "addmine", "list", "delobj", "addfire", "addsmoke", "addblock", "setlinked", "loadmap", "savemap", "setmapsize", "clearmechs", "@mapemit", "addhex", "addice", "delice", "setcond", "addstuff", "removestuff", "clearstuff", "@btech", "btech inspection", "setvrt", "setwbv", "cargo-point"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# @btech

Inspect the BattleTech foundation:

```text
@btech status
@btech weapon-settings IS.MediumLaser
@btech setvrt IS.MediumLaser 25
@btech setwbv IS.MediumLaser 46
@btech skill-threshold Piloting-Biped
@btech skill-threshold Piloting-Biped=3000
@btech template JR7-D
@btech template-check JR7-D
@btech loadout JR7-D
@btech unit-create #44=JR7-D
@btech unit-place #44=#43,0,0
@btech unit-remove #44=#2
@btech unit-towable #44=on
@btech mapfile test.map
@btech inspect #42
@btech range #44,#45
@btech map-create #43=test.map
@btech map-reload #43=test.map
```

`template` reads a biped BattleMech definition from the configured mech directory.
`mapfile` reads source terrain and environmental settings from the configured map
directory. `inspect` reports saved map or unit metadata for a database object.
`status` reports the loaded identity counts and implementation status.

Map assets containing `&` fire terrain automatically enable permanent fires
(map flag 8). An explicit `flags: gravity temperature` line overrides that
default; retain flag 8 to preserve those fires when saving. Temporary-fire `>`
and smoke `:` markers load as grassland.

Unknown terrain characters also load as grassland, retaining their elevation.
Map creation and loading report each substitution to the ordinary `MapErrors`
channel, when that channel exists. Diagnostics participate in the load transaction;
an aborted load leaves neither terrain changes nor channel messages. Malformed
dimensions, rows and elevations are still rejected.

Inspection commands are read-only. All `@btech` operations require Wizard authority.
No unit or map becomes active as a result of inspection.

To create a DEBUG command object, use an existing Thing you control:

```text
@btech/register #42=DEBUG
@btech/info #42
@btech/unregister #42
```

The switches may be abbreviated to `/r`, `/i`, and `/u`. Bare `@btech #42` also
reports its registered type. Registration preserves the object and its location;
unregistration removes its DEBUG role. A DEBUG object supplies its commands when
it is the actor, the actor's location, or carried by the actor. Its `SETWBV`
command is public, as in the reference; other DEBUG controls require Wizard access.
Registration and removal survive saving and restart.

`@btech/register #43=MAP` initializes a live Thing as a 21 by 11 grassland map
named Default Map. Carry it or enter it to use `VIEW 10 5`, `LOADMAP <filename>`
and other map commands. Registering the same type again preserves the existing
map, including edited terrain. The initial map and subsequently loaded terrain
survive saving and restart. `@btech/unregister #43` shuts down and detaches the
map's supported tactical units, then removes its terrain, events and MAP role.
It preserves the game object, inventory and physical contents. Cleanup notices
go to GOD; the invoking wizard receives the unregistration confirmation.
External entrance/exit markers remain attached to their target object, while
authored links to the removed map are cleared. Initialization and teardown for
The legacy AUTOPILOT registration role does not manage the new unit-attached
controller; use the in-game `btech.autopilot` Lua API. Use the unit-creation
commands above for supported constructed units.

Explicit unregister/register changes between DEBUG and MAP can be committed in
one save.

Supported Mechs, ground vehicles and VTOLs have movement and combat controls;
see `help piloting` and `help flight`. Full gameplay parity remains under
development. Ground autopilots are available through in-game Lua; repairs and
loading units into other units are deferred. A successfully parsed
template has not yet had its equipment and rule support validated. Use
`template-check <name>` to preview the unit-construction checks, the first
rejection reason and any ammunition normalization. A successful check means the
template can be constructed under the currently implemented construction rules; it
does not establish complete gameplay parity. Maps without a saved terrain dictionary require explicit `map-reload` before
tile queries are available. Reload replaces terrain from the asset; it cannot
recover saved terrain edits. It requires unchanged dimensions, no units on the
map. Supported map objects are retained; unrecognized object types prevent saving
the reload. Terrain and mine/building changes invalidate saved lookup caches.

`map-create` registers an existing room or thing. Both map operations require
control of the target and save terrain and environment together. A failed save
leaves the map unchanged. These operations do not enable combat simulation.

`loadout` groups supported equipment into weapons, ammunition bins and system
criticals, rejecting unresolved equipment or modes. It does not create a unit or
validate all chassis and gameplay rules.

`unit-create` constructs a persistent Mech or ground vehicle on an unused live thing
you control. For example, `@btech unit-create #44=Demolisher` creates a ground vehicle.
Vehicles support saved state, administrative placement/removal, cockpit assignment,
startup/shutdown, and speed/heading controls on supported terrain, one-level slopes
and bridge decks. Hovercraft can travel over water or ice and beneath clear bridge spans.
Low bridge spans use vehicle clearance checks.
Vehicles trigger mines as they enter covered hexes, using their current mass and
height. Terrain crossings use elevation, collision, flooding and fire rules. The unit retains its definition across restarts. Players use `enter`, `pilot`, `unpilot` and `leave`; supported cockpit combat is described in `help piloting`.

`unit-place` assigns zero-based hex coordinates and moves the unit into a decoded
map. Control of both objects is required. `unit-remove` clears placement and moves
the unit into an ordinary room or thing. Remove units before reloading terrain or
moving them through ordinary world commands. Placement preserves unit condition.

`range` reports geometric distance and bearing between placed units on one map.
It distinguishes horizontal range, spatial range including height/depth, and
adjacent hex steps. It does not perform line-of-sight or weapon checks.

`map-conditions <map>=<night|twilight|day>,<visibility>` sets saved battlefield
lighting and weather visibility (0–60 hexes). For example,
`@btech map-conditions #43=night,15`. This requires Wizard authority and control
of the map. Units may remain on the map; terrain is unchanged. `inspect` displays
light as 0 (night), 1 (twilight), or 2 (day), plus visibility and the line-of-sight
ceiling. Sensors reach fifteen hexes in any conditions; beyond that, visibility
sets how far units see (see `help line of sight`). Failed saves leave the previous conditions in effect. Moving a map into or out of night switches running automatic searchlights on that map on or off.

`@btech inspect` includes a placed unit's signed elevation. This follows its
current altitude, including jumping and height retained after terrain collapses.

An interrupted bridge collision can leave the continuous position ahead of the
stored hex. `inspect` labels this pending hex update until a supported movement
step, jump step, placement or removal completes it.

Unit inspection also reports current gameplay mass. Lua unit state includes its
component breakdown in 1/1024-ton units, derived from construction and damage.

`skill-threshold` reads or changes a runtime XP threshold. Names and short aliases are case-insensitive. Values range from 0 through 2147483647; zero disables earned skill levels. Changes apply when experience is next awarded, and defaults return after a database reload.


Radio diagnostics use ordinary administrator channels when those channels exist.
`MechFreqs` reports positive frequency settings that match a different-team unit on
the same map. `ZeroFrequencies` records transmissions on frequency zero when the
battlefield map is in-character. Failed actions leave no partial diagnostic history.


Mechs and ground vehicles share C3/C3i connections, messages, reports and range
assistance. Vehicle command computers use one equipment slot each.

`c3i <ID>` connects your running C3i-equipped unit to a visible friendly unit's
network. `c3i -` disconnects. A network holds six units; disconnect before joining
a different one. Hostile ECM prevents changing connections. Shutdown preserves
membership, while loss of the computer, a team change, or leaving the map removes
it. C3i uses the closest usable member for aiming range, while your own weapon
reach, minimum range, and firing visibility still apply.

`c3imessage <text>` sends to available members of your C3i network and echoes to
your cockpit. Shutdown, hostile ECM, and unconscious pilots prevent reception.
Radio tuning and sightlines do not restrict network messages.


`c3inetwork` privately lists your available C3i peers, their positions and motion,
and remaining armor and structure. It works without visual contact. Shutdown or
ECM prevents a peer from reporting; an unconscious pilot does not.


`c3itargets` privately lists targets seen by you or available C3i peers. `r:` is
physical range; `c:` is shared targeting range. P/S markers describe your own
sensors. Peer sightings can identify targets, but do not grant your unit a firing
lock or replace normal firing visibility.


`c3 <ID>` joins a visible friendly classic C3 network; `c3 -` leaves. Each working
master computer contributes three peer slots, with a twelve-unit total limit.
Two slaves need a master before they can connect. C3 and C3i memberships are
independent. Classic C3 now supplies aiming range assistance and takes priority
over C3i when both are connected. Unavailable masters reduce usable capacity.



`c3message <text>` sends to available classic C3 peers and echoes to your cockpit.
Shutdown, hostile ECM, or an unconscious pilot prevents reception. Unavailable
masters reduce message capacity without removing saved membership. C3 and C3i
messages use separate audiences.

`c3network` privately displays classic C3 peer status; `c3targets` displays targets
seen by you or available classic C3 peers. These use the same columns as their
C3i counterparts. Shutdown and ECM reduce available master capacity; unconscious
pilots still supply automatic status and sightings. Saved membership is retained.

Radios with extra capabilities can use `setchannelmode A=DI` to display relay
paths on digital messages, or `setchannelmode A=S` to search for unknown analog
transmissions. A scanner gradually adjusts its frequency toward detected traffic.
Scanning is shown as `S` in `listfreqs`. Available channels and modes depend on
the unit's radio; analog-only hardware cannot use digital messages.

The Speed Demon advantage increases ground acceleration and braking by 25 percent.
It does not increase the unit's maximum speed.


Ground vehicles with artillery can select a hex and use `fire <weapon index>`.
The shot consumes ammunition, adds weapon heat, and arrives after its flight time.
Cluster, smoke and mine payloads and hotload jams follow the artillery rules.
Friendly Mechs and vehicles can automatically observe misses and improve subsequent
artillery aim. Selecting a new target clears that correction. Vehicles also use
`spot #your-unit` to declare spotting, `spot #observer` to select a friendly acquired
observer, and `spot -` to clear either role. A spotter cannot fire. Linked missiles
use the observer's acquired unit target; artillery uses its visible selected hex.

`map-wrapping <map>=<on|off>` enables or disables opposite-edge wrapping. For
example, `@btech map-wrapping #43=on`. This requires Wizard authority and control
of the map. The setting is saved across restarts and applies to ordinary Mech,
ground-vehicle and VTOL movement. It does not link two different maps.

Wrapping also applies to projected jumps. You cannot disable it while an active
jump needs to cross a map edge; let the jump finish before changing the setting.

Use `@btech unit-towable <unit>=on|off` to set scenario permission for towing an
out-of-character unit. This works for Mechs, ground vehicles, and VTOLs. It does
not override pickup equipment, visibility, movement, or team restrictions.
Turning it off prevents later out-of-character pickups; it does not detach an
existing tow. `@btech inspect #unit` reports the permission and both directions
of tow ownership.

Use `@btech unit-fortified <unit>=on|off` to set scenario fortification. Stop the
unit, finish turning, land aircraft, release tow cables and cancel building entry
before enabling it. Fortified units cannot move, change heading, take off, jump,
dig, change hull-down posture, enter hangars or participate in pickup. They count
as immobile targets for aiming. This setting survives shutdown and restart.
`@btech inspect <unit>` reports the flag.


Use `@btech unit-observer <unit>=on|off` to assign the observer role to a Mech,
ground vehicle or VTOL. Observers receive exact scan details, projected-map range
exemptions and untuned radio monitoring. They cannot send targeted radio and do
not announce routine contact changes; weapon-lock loss still warns. The role
survives restart and appears in `@btech inspect <unit>`. Contact acquisition and
terrain visibility still apply; this is not a general visibility bypass.

## Reactor explosion scenario action

A trusted Lua callback can use `btech.unit.reactor_explode(unit)` to detonate a
constructed Mech. It returns the ordered blast hits, ignited hexes and
notices. An optional `section_explosion` records an earlier instability
blast caused by destroying the reactor itself. Damage, crew evacuation and notifications roll back together
if the callback fails. This scenario action bypasses cockpit self-destruct
permission and countdown settings. Pilot-owned sequences use
`btech.unit.explode(unit, pilot, argument)`. The scenario flag
`btech.unit.explode_safe(unit, enabled)` protects against new ammunition requests
without canceling an admitted sequence. Ejected MechWarrior infantry is deferred.


With `btech_stackpole` enabled, destruction or flooding of an engine compartment
can trigger this blast when engine losses reach three and the reactor is running
or starting. An ordinary third engine critical does not perform this check.
The instability window includes
30 seconds after the first center-torso internal damage; the hit opens that
window after resolving its criticals. New worlds have the same initial grace.
Windows expire while stopped and survive restarts. Trusted unit state exposes
`reactor_instability_remaining` for scenario inspection.

## Fully destroyed wrecks

In-character Mechs and vehicles whose remaining internal structure is entirely
lost retire after ten seconds. Their radio channels and TIC groups clear when
the event starts. Ordinary wrecks with surviving structure remain on the map.

Retirement moves the surviving game object to the configured used-Mech store,
removes its BattleTech registration and sets Going, Dark and Zombie. The source
leave action runs before retirement; the unit's move action runs afterward.
Quiet teleport suppresses the ordinary arrival announcement. A failed callback,
blocked departure or failed save rolls the retirement back for retry. The timer
survives restart, and changing the IC flag does not cancel an admitted event.


Use `@btech unit-weapons-hold <unit>=on|off` to prohibit or restore cockpit
weapon fire for a Mech or vehicle. Hold also rejects TIC groups before their
arguments are decoded and preserves a hidden unit's cover. It does not change
mechanical readiness or ammunition. `status info` displays `WEAPONS HOLD` while
active. The setting survives shutdown and restart. Trusted Lua uses
`btech.unit.weapons_hold(unit[, enabled])`; omitting the value reads it.


`@btech unit-visibility <unit>=normal|invisible|clairvoyant|both` replaces a unit's
operator visibility flags. Invisible units cannot be acquired by ordinary sensors.
Clairvoyant units can inspect contacts and terrain through visibility restrictions,
including invisible units, but this does not provide a sensor lock or ordinary
weapon aim. Both flags are independent of observer mode and exclude the unit from
opponents that prevent HIDE preparation. The flags persist through restart and
apply to Mechs and supported vehicles. Trusted Lua uses
`btech.unit.visibility(id, {invisible = true, clairvoyant = false})`; omit the table
to read the current flags.

`@btech unit-combat-safe <unit>=on|off` grants or removes damage immunity for a constructed unit. Trusted Lua uses `btech.unit.combat_safe(unit, enabled)`. The flag survives restart. Map building flag 2 also suppresses damage originating inside that map; entrance safety flag 8 does not grant immunity.

## Runtime weapon settings

`weapon-settings <weapon>` shows the current recycle time and Battle Value.
`setvrt <weapon> <seconds>` accepts 1 through 127 seconds; `setwbv <weapon> <value>`
accepts 0 through 2147483647. Use canonical weapon names such as `IS.MediumLaser`
or `CL.LRM-20`, or exact manufacturer-qualified names such as
`Magna.IS.MediumLaser`. Manufacturer names must match the weapon family; the
setting still applies to every installation of that weapon. Names ignore ASCII
case and do not accept wildcards. An equals sign can replace the space before the value.

These Wizard settings apply to every supported chassis. New weapon and AMS
activations use the recycle override, while active countdowns keep their remaining
time. Weapon specifications, unit Battle Value and Battle Value experience use the
same current values. Restart restores catalogue defaults; an existing countdown
still finishes normally even if it exceeds the restored default.

Trusted callbacks can inspect with `btech.weapon.settings(name)` and change values
with `btech.weapon.set_recycle(actor, name, seconds)` or
`btech.weapon.set_battle_value(actor, name, value)`. Both setters require a Wizard
actor and participate in callback rollback.

## Parts inventory administration

`@btech inventory <object>` lists an object's loose-parts stock by the identifiers
stored in the game database. `@btech inventory-set <object> <part> <manufacturer> <quantity>`
corrects one quantity. Manufacturer identifiers range from zero through five;
quantity ranges from zero through 2147483647. Zero removes the entry. These
Wizard operations share one stock model for rooms and units.

Trusted callbacks inspect `btech.inventory.read(object)` and make Wizard stock
corrections with `btech.inventory.set(actor, object, part, manufacturer, quantity)`.
Changes persist in the existing economy table and roll back with the callback.
Part arguments also accept exact stock names such as `Gold`, `HeatSink`,
`IS.MediumLaser` and `Ammo_IS.LRM-20`, ignoring ASCII case. Inventory listings
include those names. `Steel` and `CASE-II` each name more than one catalogue
record; use a numeric identifier to select those entries. Lua can use `btech.inventory.set_named(...)`, inspect a
part with `btech.inventory.part(name_or_id)`, and read physical stock weight
with `btech.inventory.mass(object)`.

Carried stock now contributes to the shared unit-load calculation and throttle
limits. CargoTech halves its movement penalty; ordinary Mechs carry twice the
physical stock mass for speed accounting, while ordinary vehicles carry its full
mass. Towing bonuses do not discount cargo. Removing stock restores the unloaded
ceiling. Wizard corrections immediately reconcile the unit's speed limits and
log the actual addition or removal to an existing `MechEconInfo` channel.
Assigning an unchanged quantity emits no record. Stock, speed changes, channel
history and notifications roll back together if correction or publication fails.
These controls do not install equipment or perform cargo loading.
Use `manifest`, `stores`, `loadcargo` and `unloadcargo` for cockpit stock handling;
see `help cargo`.

## Cargo transfer points

`@btech cargo-point <map>` inspects a map's loading location.
`@btech cargo-point <map> <x> <y> [hide|reveal]` sets it; the default is `hide`.
`@btech cargo-point <map> clear` removes the location restriction. Coordinates
must lie inside the map. The hint setting controls whether a failed location
check gives the required coordinates to the player.

Trusted Lua reads `btech.map.cargo_point(map)` and configures a point with
`btech.map.set_cargo_point(actor, map, {x = 2, y = 1, reveal_hint = false})`.
Pass `nil` to clear it. Edits require a Wizard, participate in callback rollback,
and survive restart and same-size terrain reload. Cockpit loading checks this point; unloading does not.

## Cargo-space construction

Mech and vehicle templates accept nonnegative `Cargo_Space` values in hundredths
of a ton. The installation contributes construction mass independently of its
loose-stock contents: ordinary installations divide capacity by 500, CargoTech
by 100, and Carrier_Tech by 1000 before conversion to 1/1024-ton mass units.
Carrier_Tech takes precedence when both flags are present. Fractional mass is
truncated after conversion. Invalid or overflowing capacities are rejected.

Lua `btech.unit.state(unit).mass.cargo` reports this installation mass; inventory
mass remains available separately. Cargo installations participate in shared
load-based movement limits even while empty. Suit capacity and loading another
unit remain unsupported.


Successful cockpit cargo transfers emit two records to an existing
`MechEconInfo` channel: the unit's stock change, then the hangar's stock change.
The records use the actual quantity moved. Ordinary channel listeners and
history settings apply. The commands do not create the channel automatically.
Transfer failures publish no records; channel publication failures also roll
back the transfer. Lua cargo transfers use the same transactional behavior.


## VTOL fuel

`@btech fuel <unit>` inspects fuel. `@btech fuel <unit> <amount>` corrects the
remaining fuel as a Wizard, within current capacity and the 32-bit fuel counter.
Trusted Lua uses `btech.unit.fuel(unit)` and
`btech.unit.set_fuel(actor, unit, amount)`. The same fuel projection appears in
`btech.unit.state(unit).fuel` and the cockpit status report.

Each installed `Fuel_Tank` critical and each carried tank, across all
manufacturer records, adds 2000 units of capacity. Installed tanks contribute
cargo mass and retain their capacity and mass when their slots are damaged. Loading a tank does not add fuel. Unloading tanks can leave fuel above
the new capacity; that fuel remains aboard and survives restart. Fuel above the
template's original capacity adds cargo mass and receives the unit's usual cargo
mass adjustment. Consuming fuel reduces that extra mass. Ground vehicles and
Mechs retain ordinary stock mass for fuel tanks and gain no flight-fuel state.

Refilling does not repair rotor damage or cancel a fall. Installed tanks and loose tank stock share the same fuel and load calculations.


## Catalogue stock commands

Wizards can use `addstuff <pattern> <count>` and `removestuff <pattern> <count>`
to change stock in their current location. `clearstuff` removes all stock there,
including unknown imported part identifiers. These tools do not require a
running unit or cargo-loading access. Lua targets an explicit holder through
`btech.inventory.add(actor, object, pattern, count)`, `.remove(...)` and
`.clear(actor, object)`.

Selection uses exact abbreviations, then full catalogue names, then wildcard
names. Additions can create stock that is not already present. Each positive
request is capped at 50,000 per matching catalogue entry. Wizards other than GOD
may select at most 20 entries. Removal floors stock at zero; its confirmation and
economy records retain the capped requested count, even if less stock existed.
Actuator component balances follow the economy's generic `Actuator` stock rule.

The whole selected batch, resulting unit speed limits, and `MechEconInfo` messages
commit together. Errors leave no partial batch or diagnostics. Clearing emits
one reset record, including when the inventory was already empty.


`btech.inventory.add_stores(actor, object, pattern, count)` provides the scripted
single-match stock operation. Exact abbreviations and full names take priority;
otherwise it selects the first wildcard match in long-name order. Positive
counts cap at 50,000; negative counts remove stock without that cap, flooring
stored quantities at zero. Zero succeeds without matching a part. No match
returns false. Patterns must be shorter than 2048 bytes.

Its economy record preserves the signed requested count. The stock change,
load correction and record share the same transaction, including Lua `pcall`
and late callback rollback. Use `inventory.add` or `remove` for multi-match
operator behavior.


Use `setcond <gravity> <temperature> [vacuum [underground]]` while located on a
BattleTech map to change its environment. Gravity is a percentage of Earth
gravity, from 0 to 255; temperature is Celsius, from -128 to 127. Optional flags
accept 0 or 1. Omitted vacuum is cleared. Setting underground to 1 enables it;
this command retains underground status once enabled, even when passed 0.

Special environmental rules are enabled for vacuum, gravity other than 100,
or temperatures outside -30 through 50. Existing movement, heat and flight
rules read the changed map immediately. Active jumps keep their routes and
sample the new gravity on their next movement tick. Unit state and simulation
time are not reset. The change and confirmation share a transaction.

Lua uses `btech.map.environment(actor, map, {gravity=100, temperature=20,
vacuum=false, underground=false})` and returns the resulting values. This
Wizard action is separate from `btech.map.conditions`, which controls lighting
and visibility. Vacuum breaches disable equipment in surviving sections. Mech breaches can
empty exposed ammunition bins, cause support-loss falls, disable engines and
expose cockpit occupants. Vehicle and VTOL breaches disable equipment while
retaining stored ammunition. Leaving vacuum does not restore breached equipment.


Use `@btech map-cloud <map>=<altitude>` to set the cloud boundary. Its initial
altitude is 200; zero disables cloud obstruction. Values range from -32768 to
32767 elevation levels. Neither sensors nor sight cross this boundary, for units or
terrain. A unit exactly at the boundary is on its upper side. Radar and active
probes ignore clouds.
Lua callbacks use `btech.map.cloud_base(actor, map, altitude)`. The value is saved
with the map and survives restart.


`ADDICE <percentage>` grows ice on the map containing the wizard. Only ordinary
water freezes, with an additional shoreline check; newly frozen hexes do not
extend the shoreline until the next command. `DELICE <percentage>` melts ice,
starting at exposed edges. Newly melted edges affect later hexes in the same
pass. Both commands preserve water depth and use the map's saved random stream.
Melting occupied ice applies the normal falls, flooding and crew consequences.
Lua provides `btech.map.add_ice(actor, map, percentage)` and
`btech.map.remove_ice(actor, map, percentage)`. Each returns changed coordinates
and any surface-break reports. Terrain, dice, occupant effects and notifications
roll back together if the action fails.

`ADDHEX <x> <y> <terrain> <elevation>` changes one base tile on the wizard's
current map. Use a map terrain symbol (`.` for grassland); elevation is converted
to a positive magnitude and capped at nine. Units retain their physical altitude
and current movement or flight state. Editing ice into water is a direct terrain
edit; use `DELICE` to melt ice with normal occupant falls and flooding. Temporary
fire/smoke overlays remain independent of the underlying tile. Lua offers
`btech.map.set_hex(actor, map, x, y, terrain, elevation)` and returns the previous
and resulting tiles. Occupied maps can be edited and saved without reloading
their source assets.

`@MAPEMIT <message>` broadcasts to the occupants of running units on the wizard's
current map and privately confirms `Message sent!`. Unconscious crews do not receive
the message. No sensor contact
or line of sight is required. Players standing directly in the map room are
outside this cockpit audience. Lua provides `btech.map.emit(actor, map, text)`,
returning eligible unit dbrefs in battlefield slot order. If delivery fails, all
staged messages are discarded together.

## Clear tactical map membership

`CLEARMECHS` shuts down every unit on your current map and removes its tactical
coordinates and contacts. Units and their occupants remain in the map room.
External tow links are released. Moving shutdown can cause falls and crew injury;
all changes and messages roll back together if the operation fails. Terrain,
map settings and objects remain intact. The optional command argument is ignored.
Trusted Lua uses `btech.map.clear_units(actor, map)` and receives removed unit
numbers in map-slot order. The actor must be a wizard.

## Resize a map

`SETMAPSIZE <width> <height>` resizes your current map to dimensions from 1 through
1000. It copies overlapping visible terrain and fills new cells with level grass.
Map objects, temporary effect timers, wrapping and building return links are
cleared. Units keep their coordinates; a resize that would leave a unit or active
map event outside the new bounds fails without changes. Clear or move units first
when shrinking past them. Lua uses `btech.map.resize(actor, map, width, height)`.

## Save a map asset

`SAVEMAP <name>` exports your current map to its configured map directory. Existing
files are replaced atomically after the world transaction commits. Temporary fire
reloads as grass; smoke exposes underlying terrain. Stale fire/smoke is cleaned
from the live map. Relative subdirectories must already exist; destinations outside
the map directory and symlinks are rejected. `Saving complete!` confirms the file
replacement; a write failure preserves the previous file and reports an error.
Stale-effect cleanup is already committed if the later file write fails.
Lua `btech.map.save(actor, map, name)` returns true when queued. A failed callback
or world save discards the queued write.

## Load a map asset

`LOADMAP <name>` replaces your current map's terrain, dimensions and asset
conditions, generates bridges, and removes map objects. GOD (#1) keeps units on
the map; other wizards shut down and clear units after loading. Shutdown falls
therefore use the newly loaded terrain and conditions. Units keep their physical
altitude until movement resolves it. Other map settings, including cloud base,
remain intact. Invalid assets or crops that exclude placed units fail atomically;
use `CLEARMECHS` first for such crops. Lua uses `btech.map.load(actor, map, name)`.

## Link opposite map edges

`SETLINKED` enables wrapping on your current map. It preserves terrain, units and
other map objects. Each call adds a linked marker and keeps wrapping enabled. The optional argument is
ignored. Use `@btech map-wrapping <map>=off` to disable wrapping; trusted Lua uses
the existing `btech.map.wrapping(map, enabled)` operation.

## Restrict landing

`ADDBLOCK <x> <y> <distance> [team]` adds a circular landing restriction on your
current map. A nonzero team is exempt; omitting it blocks every team. Negative
distances retain an inactive restriction.
Lua uses `btech.map.add_block(actor, map, x, y, distance, team)` and returns its
restriction slot. Terrain and units are unchanged.

## Add fire or smoke

`ADDFIRE <x> <y> <duration>` and `ADDSMOKE <x> <y> <duration>` replace the marker
at a coordinate on your current map. Zero duration creates a permanent effect.
Timed smoke expires after the requested seconds, with a minimum of one tick.
Timed fire always reaches its first wind-driven spread and expires afterward;
its spread budget is clamped to a signed 16-bit value. Extremely negative fire
budgets wrap positive at a spread and can therefore burn for a long time. Off-map coordinates change nothing.
Lua uses `btech.map.add_fire(actor, map, x, y, duration)` and
`btech.map.add_smoke(actor, map, x, y, duration)`.

Saved fire and smoke records retain their restoration terrain without restarting
spread or expiry timers. New effects at the same coordinate replace those records.
`LIST OBJS` includes both stored restoration records and running effects.

## Delete map objects

`DELOBJ <type>`, `DELOBJ <x> <y>`, or `DELOBJ <type> <x> <y>` removes matching
map records. Types are FIRE, SMOKE, DECO, MINE, BUILDING, LEAVE, ENTRA, LINKED and
BLZ. Case-insensitive prefixes are accepted in that order. Generic decorations
restore their recorded terrain; removing a building entrance clears its interior
return links. Removing one linked marker leaves wrapping enabled if another remains.
The reported count covers selected records, excluding reciprocal cleanup.

Lua uses `btech.map.delete_objects(actor, map, type, x, y)`; omit type to select all
kinds at a coordinate, or omit both coordinates to select a type across the map.
Rust derives mine/building lookups from their definitions. `DELOBJ TBITS` reports
that there is no separate lookup cache to delete and leaves those definitions active.

## List map contents

`LIST MECHS` shows supported units in saved battlefield-slot order, including
vehicles and VTOLs. `LIST OBJS` shows owned map objects in type and slot order,
with coordinates and relevant timer, restoration, destination or restriction
details. Prefixes such as `LIST M` and `LIST O` are accepted. Listing does not
advance timers, roll dice, discover contacts or change map state.

Lua uses `btech.map.list(actor, map, target)` to publish the same report.

See `help updatelinks` for rebuilding building routes from saved map configuration.


## Add mines

`ADDMINE X Y TYPE STRENGTH [EXTRA]` adds a mine owned by you. Types are Standard,
Inferno, Command, Vibra and Trigger; use the full name. Extra defaults to zero and
sets the command channel, vibra threshold or trigger radius for those types.
Coordinates must be on the map. Strength is stored between -32768 and 32767;
confirmation shows the requested value. Multiple mines may occupy one coordinate.
New mines are processed before older mines, including mines placed by artillery.

Lua uses `btech.map.add_mine(actor, map, x, y, type, strength, extra)` and returns
the saved record slot. Placement does not immediately trigger the mine.


## View map terrain

`VIEW X Y` shows a labelled terrain map centered at the requested coordinate,
clamped to the map edges. It uses your `MAPDISPLAY` dimensions and color preference.
This Wizard view needs no cockpit or working sensors and shows no unit markers.

`btech.map.view(actor, map, x, y)` publishes the same view and returns its styled
text and clipped viewport. Viewing does not acquire contacts or change simulation
state. Coordinates outside the map select the nearest edge.


## Edit map fields

`@SETMAP field value` edits a field on the current map without a confirmation reply.
Field names
match in full without regard to case. Lua uses
`btech.map.set_field(actor, map, field, value)` with the same wizard authority.

Writable fields are `cf`, `cfmax`, `regen_factor`, `gravity`, `temperature`,
`maplight`, `mapname`, `mapvis`, `winddir`, `windspeed`, `cloudbase`, `flags`
and `sensorflags`. Dimensions, `maxvis`, `buildonmap` and `firstfree` are read-only.
`sensorflags` switches perception off for everyone on the map: bit 0 (`a`, value 1)
disables the sensor band, bit 5 (`f`, 32) radar and bit 6 (`g`, 64) active probes.
Other bits have no effect.
Light accepts 0–2 and visibility 0–60. Wind direction must be 0–359 and speed
nonnegative. Integrity must stay between zero and its maximum; set the maximum
first when creating a structure. Numeric input must fit a signed 32-bit integer.
Short fields clamp to signed 16-bit before domain checks; temperature clamps
to -128–127. Generic gravity editing also clamps to a signed byte, then stores
its unsigned value: 1000 becomes 127, -1 becomes 255 and -1000 becomes 128.
Use `SETCOND` to enter gravity directly as 0–255. Names retain at most 29 UTF-8 bytes without
splitting a character. Renaming does not reload terrain.

Bitvectors accept a signed integer or letters `a`–`z`, then `A`–`F`, for bits
0–31. `!` clears the following letter from a value constructed from zero;
`ab!a` yields only bit `b`. Invalid letters or incomplete negation reject the
edit. Environment field edits preserve flags; use `SETCOND` for its combined
condition and flag update. These edits do not advance time or schedule repairs.


## Check map consistency

`FIXMAP` checks the current map's membership and world invariants, then reports
how many units were checked. Lua uses `btech.map.check(actor, map)` and returns
the map ID and checked unit IDs in map-slot order. Both require wizard authority.
Rust derives membership from unit placements; there is no separate pointer index
to rebuild. An inconsistent snapshot produces an error. The check does not move,
remove or shut down units, advance timers, or alter saved slots.


## Inspect map fields

`@VIEWMAP [1|4][prefix]` reports the current map's fields in catalogue order.
The default is two columns; an initial `1` or `4` selects that many columns.
The optional prefix matches field names without regard to case. For example,
`@VIEWMAP 1map` lists map-prefixed fields in one column. Lua uses
`btech.map.fields(actor, map, arguments)` and returns the same fields and text.
Both require wizard authority and leave simulation state unchanged.

`buildonmap` reports saved parent metadata, independently of LEAVE routes.
`firstfree` shows `n/a` because Rust has no historical pointer-array allocation
counter; the Lua field has no value. `gravity` uses the generic signed-byte
view (255 displays as -1). Flag values display all supported letters a–z/A–F,
with `-` for zero. Titles use the map object's name; `mapname` is the saved map
asset name. Use one column to see longer field labels.


## Runtime weapon settings

`SETVRT weapon seconds` changes future activations to use a recycle duration of
1–127 seconds. `SETWBV weapon value` sets Battle Value from 0–2147483647.
Both require wizard authority, accept exactly two arguments and share the
`@btech setvrt`/`@btech setwbv` controls. Weapon names are exact and case-insensitive;
supported manufacturer-qualified names select the same weapon identity.
These runtime settings apply across unit types and reset on database reload.
Existing active recycle countdowns are preserved. Lua uses
`btech.weapon.set_recycle` and `btech.weapon.set_battle_value`.


`SETXPLEVEL skill threshold` silently sets a runtime XP threshold as a wizard.
It accepts exactly two arguments; the threshold must be 0–2147483647. Zero
prevents gains. Skill names and catalogue aliases are case-insensitive; attribute
and advantage names are rejected. This shares `@btech skill-threshold` and Lua
`btech.character.set_threshold`. Existing XP is retained, with the new threshold
used on later awards. Threshold overrides reset on database reload.


## Skill XP leaderboard

`XPTOP skill` displays up to sixteen players ranked by their saved XP balance.
Wizard players and players with no recorded XP are excluded. Earned skill levels
are excluded from the balance. At most 10,000 eligible players are counted in
database order; percentages and the grand total include counted players outside
the displayed sixteen. Ties use database order. A zero total displays zero
percentages. Names and aliases are case-insensitive, and only skills are accepted.
Lua uses `btech.character.xptop(actor, skill)` and returns the same report.
Both interfaces require wizard authority and do not change XP or last-use times.


## Shut down a selected map

`SHUTDOWN map-number` shuts down and removes all tactical members of the selected
map. It requires wizard authority and uses the same action as `CLEARMECHS` and
Lua `btech.map.clear_units(actor, map)`, including shutdown consequences, towing
cleanup and transactional rollback. Supply the numeric database ID without `#`.
Trailing arguments are ignored. Units retain their world-container locations.
Without an argument, `SHUTDOWN` remains the ordinary cockpit shutdown command.


## Save the world

`SAVEDB` requests a SQLite checkpoint of the current world, including unchanged
state. It requires wizard authority and ignores trailing input. Lua uses
`btech.database.save(actor)` inside a transaction; its return value confirms the
request was queued. The server publishes “SQLite checkpoint complete.” only after
persistence succeeds. Persistence failure uses the normal save-error response;
no success message is published. Callback rollback cancels the request.


## Part name forms

`LISTFORMS` displays every part/manufacturer identity in the shared stock-name
catalogue, ordered by short name. Each row shows its zero-based report index,
short name, long name and very-long name. The report includes parts with no live
stock and uses paced output. It requires wizard authority and ignores trailing
input. Lua `btech.inventory.forms(actor)` returns the same ordered forms with
part and brand IDs as detached data; it does not publish the text report.

`eventstats` reports whether the one-second simulation currently has work,
scanner-observer count, and artillery shots in flight.
`memstats [long]` reports live map and unit counts, inline record sizes, and
exact compact JSON state size. These sizes exclude heap allocation accounting;
allocator totals are unavailable. `long` adds registration counts by type.
Lua exposes the same detached measurements through `btech.runtime.stats(actor)`.
All three operations require wizard authority.

Inside a physical unit, `setteam <number>` changes its team. The unit must be on a
map; the value is a signed 32-bit integer, with negatives normalized to zero.
Changing teams clears its C3/C3i network assignment. The unit's hiding and
scenario-lighting state are preserved. Lua uses `btech.unit.set_team(actor, unit, team)`.

Inside a physical unit, `@losemit <message>` broadcasts an emote to running units
that currently see it. The source cockpit is excluded; you receive “Broadcast
done.” A leading apostrophe joins directly to the displayed unit name. Message
text is literal. The source need not be started. Lua uses
`btech.unit.losemit(actor, unit, message)` and returns the observer-unit count.

`@damagesection <section> <damage> <isrear> <iscritical>` applies a located hit to
the occupied physical unit. Damage must be 1–1000; signed numeric flags treat
zero as false and other values as true. Use chassis-specific section names or
abbreviations. Rear hits use rear torso armor on Mechs and redirect vehicle front
hits to rear armor. Criticals, crew injuries and evacuation use normal damage
rules. Lua exposes `btech.unit.damage_section(actor, unit, section, damage,
rear, critical)` with boolean flags. The source needs neither power nor map
placement; wizard authority is required.


`@damage <damage> <clustersize> <isrear> <iscritical>` distributes damage across
random hit locations on the occupied physical unit. Damage must be 1–1000 and
cluster size must be 1–damage. Here cluster size is the number of packets:
`@damage 11 3 0 0` applies three packets of three damage, discarding the remainder.
Signed numeric flags treat zero as false. A rear flag selects rear armor;
a rear attack direction also enables rear armor for subsequent packets. Random
hit-location rules determine critical eligibility, replacing the supplied
critical flag. Every packet is processed even if an earlier hit destroys the unit.

Lua provides `btech.unit.damage(actor, unit, damage, clusters, rear, critical)`
with boolean flags. The report includes `packet_damage`, `discarded_damage` and
an ordered `impacts` list. Wizard authority is required; power, placement and
pilot assignment are not. Damage, injuries, evacuation and notifications commit
together or are rolled back together.


`@weight` privately displays the occupied unit's construction allocation:
components, weapons, installed systems, ammunition bins, total tons and the
offset from nominal tonnage. It counts original armor and equipment, including
empty ammunition bins. Combat damage and expended rounds do not reduce this
report; current physical mass remains available in the ordinary unit state.
Trailing arguments are ignored. Wizard authority is required, but power, pilot
assignment and map placement are not.

Lua `btech.unit.weight(actor, unit)` publishes the same private report and returns
its formatted text. A failed callback or output limit rolls back the report's
messages. Weight allocation is diagnostic and does not certify design legality.


`setxy <x> <y> [z]` moves the occupied unit within its current battlefield.
Without Z it uses the destination surface (hovercraft remain above water) and
lands a VTOL. Explicit Z is clamped to -32768 through 32767. Pilot, power,
heading, commanded speed and battlefield ID are retained. Incoming weapon locks
are lost and observations are refreshed; the moved unit keeps its selected target.
Attached tow partners move together.

A jump keeps its route and elapsed progress: the assigned position lasts until
the next flight sample. An ongoing forced descent keeps its speed and countdown.
Lua `btech.unit.setxy(actor, unit, x, y, z)` accepts omitted/nil Z and returns the
committed position and elevation. Wizard authority is required. Invalid inputs,
failed callbacks and notification failures leave positioning and output unchanged.


## SETMAPINDX

`setmapindx <map dbref> [preferred ID]` assigns the occupied unit to a battlefield.
Use a decimal number without `#`. Units may be running; crew, controls and altitude
are retained. Coordinates outside the destination reset to 0,0. Destination maps
hold up to 250 units. Preferred IDs use the first two characters, normalized to
uppercase letters; occupied IDs select a free random pair. An omitted or one-character argument
uses the saved preference, then random selection when none is configured. Assigning
an ID does not reorder existing membership. Reassignment clears targeting
observations, TAG selection and command-network links, and releases towing.

`setmapindx -1` removes battlefield membership while retaining the unit's pose
and enclosing game location. Re-entry before the next update retains its controls.
A removed running unit shuts down on the next simulation update; re-entry after
that update uses the retained coordinates with systems off.

Lua `btech.unit.setmapindex(actor, unit, map, preferred)` uses the same wizard
checks, private confirmations and rollback. Its `assignment` result is nil for
removal, otherwise a table containing `position`, `label` and `reset_origin`.


The saved preference is independent of the current ID. Wizards can configure it
with `btech.unit.set_preferred_id(actor, unit, "QX")`; exactly two ASCII letters
are required and lowercase letters normalize to uppercase. Nil or empty text
clears it. `btech.unit.state(unit).preferred_id` reports the setting. Editing it
does not rename a unit already on a map or consume random numbers. The next
SETMAPINDX selection uses it unless an explicit ID overrides it. A collision
changes the assigned ID without changing the saved preference.


An active jump keeps its planned route and progress when reassigned. Its current
position follows the map-assignment coordinate rules. Later movement uses the
new map's wrapping; on an ordinary non-wrapping edge the unit stops at the edge
and lands using the normal landing rules. Saving and reloading preserves that
continuation.


### Orbital insertion

`@ood <x> <y> [z]` inserts your occupied unit at the chosen coordinates on its
current battlefield. Omitted altitude is 300; explicit altitude uses the same
signed-short range as SETXY. You must be a wizard. Detach towing before insertion;
prone Mechs, actively digging vehicles and units already in a cocoon are rejected.
Ground units receive protection based on their current mass and descend during
one-second airborne updates. VTOLs enter ordinary flight without a cocoon, with
half their maximum speed requested. Stopped VTOLs wait aloft until startup finishes.

Lua `btech.unit.ood(actor, unit, x, y, z)` uses the same action and returns the
position, altitude and drop or flight state. Errors restore placement, controls,
protection and output. Scenario insertion replaces an existing jump or free fall.
An intact cocoon gives attackers a -2 targeting modifier. A damage-entry roll
above eight diverts the complete packet into the cocoon. Firing opens it after
mechanical failure checks, including a failed Streak lock. Surviving jump jets
compensate for a breach; otherwise the unit enters free fall.

On touchdown, an attached `on_ood_land` event runs before the landing roll and
damage. Its object, enactor and cause identify the arriving unit. An event error
restores the whole airborne tick, including earlier arrivals and their output.


## Named fields

`@viewmech [1|4][prefix]` displays fields of your occupied unit. The default is two
columns; `1` or `4` selects another layout, and the prefix filters names without
regard to case. `n/a` means that field has no available projection.

`status`, `critstatus` and `mechtype` are inspectable fields whose direct writes
are not supported. Use the implemented gameplay and administrative controls for
their supported transitions; writing these fields reports an error without
changing the unit.

`@setmech team <integer>` changes the unit's team and clears stale command-network
membership. This named field accepts a signed 32-bit team value. `@setmech xpmod
<number>` changes its finite, nonnegative shooting-XP multiplier. Successful edits
are silent and preserve other settings. `@setmech fuel <amount>` sets VTOL fuel
within its current tank capacity and updates carried mass through the shared fuel
service. `@setmech mechname <name>` and `@setmech mechref <reference>` change
the saved unit identity, preserving combat state. Each accepts 1 to 128 bytes.
`tacrange`, `lrsrange`, and `scanrange` accept 0 to 127 hexes; `radiorange`
accepts 0 to 32767; `radiotype` accepts the hardware bit mask from 0 to 255.
An explicit zero persists as zero, independent of template defaults. Later sensor
criticals still reduce assigned sensor ranges.

Inspection includes current sensor ranges, radio range and
hardware bits, and VTOL remaining and original fuel capacity. Sensor damage uses
the same range reductions as cockpit scans.

`@viewspecial` and `@setspecial` select the same field service from your location:
map or physical unit. They accept the same arguments as the
corresponding type-specific field commands. All four commands require wizard
permission and reject switches.

Lua `btech.unit.fields(actor, unit, arguments)` returns the detached report and
publishes it. `btech.unit.set_field(actor, unit, field, value)` uses the same named
edits. Errors preserve state and previously staged output.

`@setmech targcomp <mode>` selects normal (0), short-range bias (1), long-range
bias (2), multiple-target tracking (3), or anti-air tracking (4). Range biases
trade a one-point benefit for a one-point penalty across the medium-range
boundary. Multiple-target tracking removes settling delay on the selected unit
and penalizes a selected target outside the forward arc. Anti-air tracking
favors jumping units, airborne VTOLs and units descending in cocoons, and
penalizes ground targets. AntiAircraft equipment improves its airborne bonus
from two points to three.

Thermal fields report the last committed sample: `heat` includes weapon and
continuous heat production, `dissheat` reports dissipation, and `overheat` reports
excess heat. `disabled_hs` reports intentionally disabled cooling; `heatsinks`
reports surviving physical cooling capacity. Ground vehicles and VTOLs do not
accumulate thermal samples and report zero for the heat fields.

Mechs also accept `@setmech heat <number>`, `dissheat <number>`,
and `overheat <number>`. Heat and dissipation edits change
the last reported sample; they do not add stored weapon heat. Excess heat must
be nonnegative and immediately affects heat penalties. `disabled_hs` is read-only;
use the heat-cutoff controls to regulate cooling. The next thermal heartbeat resumes normal sampling
and regulation. These edits do not trigger heat damage or consume dice.

`centdist` and `centbearing` report horizontal distance and bearing to the
current hex center using the same geometry as `findcenter`.

`cargospace` reports authored cargo capacity, independently of cargo-installation
mass and carried stock. `C3iNetworkSize` reports the number of other eligible
members in the unit's C3i network; an unlinked unit reports zero.

`jumpheading` and `jumplength` retain the last admitted jump's bearing and
distance after landing. Jump length uses integer field units (322.5 per hex),
while normal movement continues to use hex distances. Units that have never
jumped report zero. Turning during flight does not rewrite the launch bearing.

`unit_era` and `unit_tro` inspect saved template metadata. Missing values display
`Undefined`. `@setmech unit_era <text>` and `@setmech unit_tro <text>` update them
without rebuilding the unit; each accepts at most 24 bytes. Lua named edits
also accept an empty string, which remains empty after restart.

`numseen` counts acquired contacts on other teams, including contacts whose
identity is unknown. Friendly contacts do not count. The field reads saved
observations without triggering a scan or consuming random state.

`@setmech turret0 <dbref>`, `turret1`, and `turret2` add explicit cockpit-notice
destinations. Their initial value is -1. Positive references to available rooms
or things receive the unit's cockpit notices. Private player reports remain
private. Duplicate and self-links do not add copies, and relaying is nonrecursive.
Unresolved references remain saved and inspectable; assigning a destination does
not grant control of weapons.

`@setmech displayname Silver Fox` sets the unit's presentation name, independently
of `mechname` and `mechref`. Status and identified contacts use the override;
unidentified contacts retain their concealed name. Names can contain up to 120
bytes and survive database reload. `btech.unit.set_display_name(actor, unit, "")`
clears the override, restoring the template-name fallback. The Lua getter and
`@viewmech displayname` report the configured override, empty when unset.

`last_startup` reports the Unix time of the most recent completed startup, initially
zero. Beginning or aborting a startup does not change it. `@setmech last_startup
<timestamp>` permits a signed integer history edit without changing engine power;
the next successful startup replaces that value. Lua named-field edits use the
same validation and transaction boundary.

Ground vehicles and VTOLs also support `mechprefs SLWarn ON` and
`mechprefs AutoconShutdown ON`. SLWarn announces changes in external illumination;
AutoconShutdown includes shutdown targets in routine contact notices. Both default
to OFF and persist with the unit. These settings do not change sensor acquisition,
contact-list filtering or loss-of-lock warnings. The corresponding Lua unit
setters use the same cockpit authorization and state.

Installed vehicle and VTOL searchlights now use `slite`, including the five-second
warm-up/cool-down, the `auto`/`on`/`off` modes and the existing Lua control. Forward beams illuminate units
and terrain; status shows lamp damage and switch timing. Shutdown cuts lamp power.
Front hits can destroy ground-vehicle lamps, canceling pending switching; VTOL
lamps do not use that ground-only hit rule.

Vehicles and VTOLs support `mechprefs ArmorWarn` and `mechprefs AmmoWarn`, both ON
by default. ArmorWarn reports transitions to low, critical and breached armor;
AmmoWarn reports low supply when firing, using all installed bins for that weapon.
Turning either OFF suppresses its notices without changing damage, ammunition use
or sensor contacts. Existing Lua warning setters support the same chassis and
cockpit checks, and preferences survive reload.

`@viewmech MechPrefs` projects the unit's saved gameplay preferences as letter
bits. `@setmech MechPrefs <bits>` replaces the supported settings; decimal masks
are also accepted. Use `0` to clear them. Letters `b`, `c`, `d`, `e`, `g`, and `h`
mean searchlight warnings, automatic cliff falls, disabled armor warnings,
disabled ammunition warnings, shutdown-contact notices, and friendly-fire safety.
For example, `bcdegh!b!g` sets
`cdeh`: `!` clears a bit in the supplied value, not in the unit's previous value.

Bits for unimplemented settings are rejected atomically. This field edits the
same values as cockpit preferences; it does not maintain an independent mask.

`mechprefs BTHDebug [ON|OFF]` and `btech.unit.bth_debug(unit, pilot, enabled)`
control the retained debug preference. `MechPrefs` bit `j` exposes the same value;
bit `f` retains StandAnyway. Both survive reload. These flags currently have no
combat or standing effects in the reference, and Rust preserves that behavior.
Toggling BTHDebug leaves StandAnyway and other preferences unchanged.


`snipe <target ID> <weapons>` predicts a target's horizontal motion until shell
flight time catches up, sets that hex as your target, and fires the selected
weapons through the normal firing rules. You must be the assigned wizard pilot.
Weapon numbers accept commas and ranges. Prediction follows current orders and
stops at blocking terrain or the map edge; it does not anticipate new orders,
future damage, vertical movement, or map transfers. Each weapon still needs its
usual ammunition and readiness. Queued artillery survives a server restart.

`@viewmech bv` reports current Battle Value to two decimal places. It uses live
armor, equipment, load and configured weapon values. The field is read-only;
`setwbv` changes the weapon values used by the calculation.

`@viewmech mechdamage` reads a compact damage report: `A:section/loss` for armor,
`A(R):section/loss` for rear armor, `I:section/loss` for internal structure,
`C:section/slot` for destroyed equipment, `R:section/slot(rounds)` for ammunition
spent, and `G:section/slot(code)` for temporary weapon failures. Records are
comma-separated; an empty value means no represented losses. Section and slot
numbers start at zero. This field also appears in `btech.unit.fields`.
It is read-only and does not represent every combat condition.

`@setmech basewalkspeed <integer>` and `@setmech baserunspeed <integer>` store
reserved administrative values. Both start at zero, accept signed 32-bit integers,
and survive restart. They do not change a unit's movement limits or throttle.
Inspect them with `@viewmech base`; Lua uses the same unit field accessors.

`@viewmech hsengoverride` reads the template's engine heat-sink allocation override.
`@setmech hsengoverride <integer>` stores a signed 32-bit override, shared with Lua
unit field access. It defaults to zero and survives restart. Updating it does not
recalculate current cooling or repair damaged heat sinks.

`StaggerDamage` is read-only and reports the Mech's saved action-time stagger
value, which normally remains zero. It is separate from recent damage history.
Positive twenty-point levels add to controlled-drop piloting rolls and produce
the `STAGGERING` status banner. `unusablearcs` remains a read-only zero; it does
not report a weapon's current firing eligibility. Normal combat checks apply.

`@viewmech tankcritstatus` reports vehicle critical conditions as letters:
`a` turret locked, `b` turret jammed, `c` dug in, `d` digging, `e` crew stunned,
and `f` tail rotor destroyed. `@viewmech critstatus2` reports `a` for a damaged
hardened gyro and `b` for an unavailable installed light probe. A dash means none
of those conditions apply. Lua unit fields expose the same values.

`@viewmech status2` reads secondary status letters: `a/b` Guardian ECM/ECCM,
`c/d/e` observed disturbance/protection/countering, `f` searchlight on,
`g/h` stealth/null signature, `i/j` Angel ECM/ECCM, `k/l` Angel
protection/disturbance, `o` automatic turret, `w` fortified, `x` weapons held,
and `y` gunnery experience suppressed. Electronic observations are the last
committed values; inspecting them does not run a new electronic-warfare check.
Lua unit fields expose the same value. `@setmech status2 <letters>` edits these
settings atomically; use `-` to clear them. Equipment and chassis constraints
still apply, and each ECM suite can select only one mode. Observation edits last
until the next field refresh and do not create an ECM emitter. Pending switch
timers retain their normal behavior.

`@viewmech status` reports main lifecycle flags from current unit state, including
power, destruction, posture, facing, targeting purpose, safety, boosters and map
conditions. It uses the same letter format as `status2`; Lua unit fields expose
the same inspection-only value. The observer-dependent partial-cover bit `e`
stays clear: inspect sight results for cover against a particular target.
Reading unit status does not run or change a line-of-sight observation.

`@viewmech critstatus` reports equipment failures and critical conditions:
`a/e` destroyed/damaged gyro, `b` damaged sensors, `c` TAG, `d` hidden,
`f/r` damaged/both destroyed biped hips, `g` life support, `h` Angel ECM,
`i` C3i, `j` null signature hardware, `k` destroyed searchlight,
`l` last observed illumination, `p` heat cutoff, `q` towable,
`s` targeting computer, `t` C3, `u` ECM, `v` Beagle probe,
`w` inferno effects, `z/A` clairvoyant/invisible, `C` observer,
`D` Bloodhound probe, and `E` Mech crew stun. Equipment letters indicate
unavailable hardware, not equipment absence. Cache and per-update bookkeeping
bits remain clear. This field currently supports inspection only.

`@setmech pilotnum <player number>` assigns a live, conscious player who is
physically inside the unit. It can replace the current pilot; `-1` clears the
assignment. Invalid replacements leave the existing pilot assigned.
`@setmech target <unit number>` selects another placed unit on the same map,
restarts sensor settling and clears artillery adjustment. This administrative
selection does not require an acquired contact; ordinary firing checks still
apply. `-1` clears all targeting modes. Both fields use the same operations through
Lua unit field access and participate in callback rollback.

`@setmech cargospace <capacity>` updates the cargo installation on a Mech, ground
vehicle or VTOL. Capacity must be a nonnegative 32-bit integer whose installation
mass fits the unit mass representation. Mass and movement limits update
immediately. Existing loose stock remains aboard when capacity is reduced. Lua
unit-field edits use the same operation.

`@setmech fuel_orig <capacity>` changes a VTOL's original fuel capacity. Use a
nonnegative signed 32-bit integer. Current fuel is retained, so reducing capacity
can increase excess-fuel load. Increasing capacity does not refuel an exhausted
aircraft. Lua unit-field edits use the same operation.

`@setmech speed <kph>` and `@setmech heading <degrees>` edit actual motion on
a placed unit, preserving its requested speed and heading. Use finite speeds
and integer compass headings from 0 through 359. The resulting motion must
satisfy the unit's speed, power and mobility constraints; invalid edits leave
state unchanged. Lua unit-field edits use the same operation.

`@setmech x <column>`, `@setmech y <row>` and `@setmech z <height>` use
scenario positioning on the current map. They retain the other integer
coordinates, recenter continuous XY within the hex, and use integer elevation.
Requested controls are retained, and attached tow pairs move together. Values
must fit a signed 16-bit integer; horizontal coordinates must be on the map.
Lua unit-field edits share the same positioning operation.

`@setmech fx <value>`, `@setmech fy <value>` and `@setmech fz <value>` edit
precise position using 322.5 units per horizontal map unit and 64.5 units per
elevation level. The other axes retain their fractional values; the containing
hex updates automatically. Values must be finite and remain on the map, with
altitude between -32768 and 32767 levels. Fractional orbital-drop altitude is
rejected. Tow pairs move together. Lua uses the same operation.

`@setmech templatesp <speed>` changes the template speed used to distinguish
walking from running when calculating firing penalties. It accepts finite,
nonnegative values and does not change live mobility, actual speed or throttle.
The value survives restart. Lua unit-field edits use the same operation.

`@setmech maxspeed <speed>` edits live maximum speed without changing
construction mass or the template firing baseline. Use a finite nonnegative
value consistent with the unit's material condition and current controls.
Actuator recalculation replaces a Mech's live correction using template speed;
vehicle motive hits reduce the live value. Edits survive restart, and Lua uses
the same operation.

`@setmech maxjumpspeed <speed>` edits available jump thrust at standard
gravity without changing installed equipment or construction mass. Existing
jet damage remains; later losses reduce thrust. Mech jump limits and orbital
compensation share this value. Use a finite nonnegative speed; destroyed units
cannot receive positive thrust. Values that exceed the flight model at low
gravity are rejected. Lua uses the same operation.

`@setmech pilotdam <0–6>` edits tactical crew injuries. Nonfatal edits retain
consciousness and pending recovery timing without rolling dice. Six injuries
apply ordinary fatal crew cleanup. This cannot revive a destroyed unit or edit
an assigned in-character pilot's RPG health. Lua uses the same operation.

`numseen` is a read-only count of acquired enemy contacts. It follows current
observations and teams; it cannot be changed independently of contacts.


`@setmech realweight <integer>` sets current gameplay mass in 1/1024-ton units
(0 through 2,147,483,647). The correction affects movement load, towing and
mass-dependent combat, and survives restart. Protection damage or ammunition use
and loss discard it; mass then follows surviving construction again. Component
weight reports continue to show physical material. Lua unit field access uses
the same edit and validation.


`@setmech tons <integer>` changes nominal tonnage while retaining equipment,
protection, damage and crew. Construction mass and movement load are recalculated;
a separate `realweight` correction is retained. Tonnage must be positive and fit
the supported construction rules (Mechs: 20–100 in increments of five). Invalid
edits leave the unit unchanged. Native and Lua edits survive restart.


`@setmech critstatus2 <letters>` edits hardened-gyro protection (`a`) and
light-probe failure (`b`); `-` clears both. Gyro edits retain existing impairment
and change whether the next hit consumes the extra protection. Probe edits
change operation without replacing damaged slots; a fresh critical disables the
probe again. Restoring operation requires installed hardware in usable sections.
Edits retain material, survive restart, and share validation with Lua unit fields.


`@setmech tankcritstatus <letters>` edits vehicle conditions: `a` turret locked,
`b` turret jammed, `c` dug in, `d` digging, `e` crew stunned, and `f` tail rotor
destroyed. `-` clears them. Flags retain material, turret heading and pending
completion/recovery events. Setting digging or stun without a timer leaves that
condition active until cleared; it does not start a timed dig or stun action.
Cover and digging can coexist, as can turret lock and jam. Chassis and hardware
constraints still apply. Native and Lua edits share rollback and persistence.


`@setmech mechmovetype <name>` changes locomotion while retaining equipment,
damage and crew. Mechs accept Biped or Quad; ground vehicles accept Track, Wheel,
Hover or None; VTOLs accept VTOL or None. Names ignore case. Equipment must fit
the new anatomy and current live state must remain valid. Invalid edits leave
the world unchanged. Native and Lua edits share validation and persistence.


`@setmech jumpheading <0–359>` and `@setmech jumplength <integer>` edit a Mech's
course. Length uses 322.5 units per hex and accepts signed 16-bit integers. During
a jump, edits redirect the remaining route without moving the current position
or resetting progress. Length means total distance, including distance already
flown. A nonpositive or already-completed length schedules landing at the current
position on the next tick; extending it before that tick can resume the route.
Heading alone does not cancel a pending landing. Normal collision and DFA
landing rules still apply, and invalid range, climb or map geometry is rejected.
Grounded edits retain values without launching; a new jump sets its own course.
Vehicles do not have an editable conventional jump course. Native and Lua edits
share rollback and persistence.
