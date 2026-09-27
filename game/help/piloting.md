+++
title = "Piloting BattleMechs"
description = "Enter a unit and take or release its cockpit"
keywords = ["status", "view", "markings", "ap", "safety", "mwsafety", "mml", "hide", "explode", "self-destruct", "usebin", "heatcutoff", "addtic", "deltic", "cleartic", "listtic", "firetic", "hulldown", "dig", "pickup", "dropoff", "enterbase", "pilot", "unpilot", "piloting", "cockpit", "startup", "shutdown", "heading", "speed", "rottorso", "fliparms", "sensor", "contacts", "lock", "stand", "prone", "lrs", "lrsmap", "fire", "sight", "target", "weapons", "weaponstatus", "weaponspecs", "critstatus", "flamerheat", "heat", "inferno", "lbx", "cluster", "firecluster", "firesmoke", "firemine", "fireswarm", "fireswarm1", "artemis", "unjam", "stinger", "hotload", "ultra", "rapidfire", "rac", "gattling", "armorpiercing", "caseless", "incendiary", "precision", "flechette", "jump", "dfa", "death from above", "land", "mechprefs", "autofall", "ams", "pods", "removepod", "removepods", "extinguish"]
article_tags = ["show_in_index"]
+++

# Piloting BattleMechs

Use `enter <unit>` to enter a nearby constructed BattleMech, then `pilot` to take
its cockpit. Entry follows normal ENTER and LEAVE policies; cockpit access follows
the unit's USE policy. Only one player can occupy the cockpit.

`unpilot` releases the cockpit while leaving you inside as a passenger. `leave`
exits the unit and releases its cockpit automatically. Teleporting out also
releases it. Pilot assignments survive a server restart.

After taking the cockpit on a battlefield, use `startup`. The startup cycle
runs for 30 simulation seconds and reports six stages. Wizards can use
`startup override` for a five-second cycle. `shutdown` stops the engine or aborts
startup, and releases the cockpit. Use `pilot` again before restarting.
Excess heat above 30 prevents startup, including the operator override; let the
unit cool before retrying.

Startup progress survives restart. It pauses while the server is offline or
unable to save progress. A running unit accepts `heading <degrees>` and
`speed <kph|stop|walk|run|back>`. Turns and speed changes are gradual. Shutting down above 10.75 kph forward
causes a fall and can cause crowding collisions. Reverse or slower motion stops
without that shutdown fall. Motion supports grassland, roads, forests, rough ground, mountains, snow and
bridge decks, including one- and two-level elevation changes. Forward steps reduce
speed by 10.75 kph per level while preserving your requested throttle. Backing
across a step normally requires a piloting check: failure causes a fall, with an
uphill fall returning you to the previous position. When reverse-step checks are
disabled, backing uses the same speed costs as forward movement. Map edges and
unresolved altitude transitions stop the unit. Buildings and walls use their mapped heights. At a steeper cliff, a successful piloting check stops you before the edge or wall.
Failure causes a fall; downhill fall damage scales with the drop. Speed affects
the check, and the server can select alternate skid rules.

Crowded hexes can cause collisions when you enter them. More than two friendly
units, or more than six total units, can trigger the configured collision rules.
Ground damage depends on current mass and relative speed; avoidance rules can
stop you or cause a fall. Landing among three friendly ground units can also
trigger crowding; landing impacts use current mass and remaining jump thrust.

Water depth affects piloting checks. Entering a submerged hex requires a
piloting check; running into water makes the check
harder and limits forward throttle to walking speed. Slipping causes a fall.
Water can flood breached sections, disable equipment and cause loss of balance.
Running commands are rejected while submerged or in high-water terrain. High
water uses its mapped height and a -2 entry modifier; slipping there causes wet
fall damage.
Bridge travel selects the deck or a lower surface at elevation -1 according to
your current height. Beneath a bridge, water checks still apply; mapped terrain
heights determine step and cliff checks.

On ice, each new hex entered from elevation zero has a one-in-six chance of
breaking. A break can drop everyone on that hex into the water. Below intact ice,
you follow the bottom and make water checks; falls and standing keep you below it.
Moving from elevation -1 into depth-one ice brings you onto its surface without
a fracture roll on that entry.
Use `mechprefs` to inspect AutoFall. `mechprefs AutoFall ON` skips the stop check
at downhill cliffs; `OFF` restores it. `mechprefs AutoFall` toggles the setting.
The setting stays with the unit through shutdown and restart. The assigned pilot
can change it with the engine off. Uphill checks and falls still work normally.
Only AutoFall is currently supported by `mechprefs`.

`pilot`, `unpilot` and `shutdown` take no arguments; these commands take no switches.

An unconscious pilot cannot take a cockpit or operate its controls. Recovery
attempts occur every 30 simulation seconds, and the countdown survives restart.
Releasing the cockpit does not clear unconsciousness. Piloting checks fail
without rolling while the pilot is unconscious; an already prone Mech retains
its automatic-success exception.

Use `jump <bearing> <range>` to engage jump jets toward a hex center. Grassland, roads,
forests, rough ground, mountains, snow, smoke, fire, water, high water, ice, bridges,
buildings and walls are supported,
including changes in elevation. Jump progress survives restart
and pauses while the server is offline or unable to save. Jet damage and gravity
limit range; jumping produces at least three heat, or one per undamaged jump MP.
Jumping and death-from-above attacks are refused when load-adjusted effective
speed is more than one MP (10.75 KPH) below the unloaded maximum. CargoTech,
movement boosters, TSM and special gravity affect that calculation. A loss of
exactly one MP is allowed. You cannot jump while towing another unit.
Wait twelve seconds after an ordinary landing before jumping again. Stabilizing adds two
to firing difficulty. Damaged landing gear or loss of consciousness can cause
a fall, and losing all jump thrust or overheating can cut the flight short.
Use `land` to attempt an early landing at your current location. A failed
control check causes a fall; a successful abort still requires any damaged-leg
or gyro landing check. Ground speed changes are unavailable in flight. Use `heading <degrees>` to turn
while jumping; facing changes do not steer the jump away from its landing hex.
Voluntary `shutdown` stops your horizontal travel and starts free fall. You drop
every three seconds, accelerating until impact. Restarting the engine does not
arrest this fall. After restarting, you can attempt `land`, but the pending fall
event still causes an impact when due.
You can fire during a jump: jumping adds three to your firing difficulty, and
a jumping target adds one plus its current jump-speed movement modifier.
Losing both legs or hips leaves you unable to stand while your jets finish the
jump; further jet loss can still bring you down.
Crossing ice during a jump can break it and drop other units standing on that hex.
Breaking downward can end your jump in a fall; breaking upward preserves your flight.
A hill taller than your current jump height can force an early landing or crash.
Water crossings use your current altitude. Submerging can flood breached sections
and improve cooling; takeoff is limited to water one level deep. Intact ice is
at surface level. Landing or falling onto it has a one-in-six chance of breaking
it, dropping units on that tile into the water. Buildings and walls block jumps
that are too low to clear them. DFA remains unavailable.
Accumulated damage does not block takeoff. Rolling stagger checks continue
during flight and can still cause a fall.

Cockpit stun blocks firing for ten simulation seconds and slows a running unit
toward walking speed. Another stunning hit restarts that countdown.

Use `rottorso <left|right|center>` (or `l`, `r`, `c`) to turn the torso. Turning
in the opposite direction first returns it to center. `fliparms` toggles arm
facing on chassis that support it. Both require a running unit and its conscious
pilot. Facing survives restart; leg weapons follow the body heading. These
commands take no switches, and `fliparms` takes no arguments.

Sensors are automatic; there is nothing to switch. `sensor` shows how far and by
what means your unit can perceive right now:

- Sensors: within fifteen hexes you detect anything with a clear line, whatever
  the darkness or weather.
- Sight: beyond that, weather visibility sets how far you see. At night an unlit
  target costs +1 to hit, and a lit one can be seen three times as far.
- Probe and radar: special equipment, if your unit carries it.

See `help line of sight` for the full rules. `sensor` takes no arguments or
switches. Lua `btech.unit.perception(unit)` returns the same report as a table.

`contacts` lists acquired targets that are currently visible, nearest first, with
unit number, chassis, friendly/hostile relation, range, bearing, heading and speed.
`contacts #unit` limits the display to one acquired target. Conscious occupants can
use this display while the unit is running. It does not roll for new contacts or
reveal positions from stale observations. `contacts` takes no switches.

Use `lock #unit` to select a currently visible contact from `contacts`, or `lock -`
to clear your target. Selection requires the running unit's conscious assigned
pilot. Sensors take eight simulation seconds to settle; selecting again restarts
that delay. A scanner update reporting your selected contact lost clears the lock.
Shutdown and battlefield removal clear selection. `lock` takes no
switches. Aimed-section commands are not yet available.

Use `lock x y` to select the unit currently occupying a coordinate. If it is empty
when you fire, your shot goes to that coordinate. Add `I` to ignite woodland, `C`
to clear it, or `H` for a terrain shot, for example `lock 5 3 I`. These terrain
modes do not directly attack occupants. H-mode hits can break ice or bridges,
causing occupants to fall. Coordinate shots require visibility and a weapon that
bears on the selected hex. Use `lock x y B` to attack the building entrance at a
coordinate. Surviving buildings can repair over time; destroyed buildings remain
at zero integrity. Command-center structures resist ordinary building fire.
Surviving damaged building interiors resume missing repair timers on load.
Saved timers keep their remaining seconds; offline time does not advance repairs.


Use `stand check` to inspect your piloting target while prone. `stand` attempts to
get up. `stand careful` adds a -2 modifier when enabled, but takes twice as long;
a failed careful attempt takes at least thirty seconds to recover. If your base
target exceeds twelve, use `stand anyway` to attempt it despite certain failure.
Failed attempts cause another fall. Successful attempts make you upright but block
forward or reverse movement until the stand timer finishes. You can use `heading`
to turn while prone, getting up or recovering from a failed attempt. Dry-ground
tactical units can stand on land or in water while at least one leg remains usable.
Ice and bridge standing still require their terrain rules. `stand` takes no switches.


Use `hide` in forest, mountains or rough terrain when your unit has camouflage
equipment, or when you are a wizard. Come to a stop first; a VTOL must be landed.
Preparation takes 51 seconds for Mechs and ground vehicles, or 41 seconds for
VTOLs. Without camouflage, wizard operators take 101 or 81 seconds respectively.
Hostile units that have acquired you can interrupt preparation. Moving into
another hex or attempting to fire cancels hiding and reveals you. Being towed
or completing an explicit building entry does not itself break cover; moving
through a map edge does. Taking armor damage reveals existing cover but does
not itself cancel preparation. Shutdown cancels preparation. Lua uses
`btech.unit.hide(unit, pilot)`.

An operator can place your unit in weapons hold. While held, `fire` and
`firetic` are rejected without breaking cover; `status info` shows `WEAPONS HOLD`.
Physical attacks can still deal damage while held, with a weapons-hold warning.

Use `weapons` to list your mounted weapons and their current readiness, recycle
seconds and available ammunition salvos. The numbers start at zero and remain
stable when equipment is disabled. Conscious cockpit occupants can inspect this
list even while the engine is off.

The assigned pilot can use `fire <number>` against the selected lock, or
`fire <number> AB` (or `#unit`) against an explicit target from `contacts`.
Battlefield IDs are case insensitive. An explicit
target does not change your lock. The weapon must be ready and bear on the
selected coordinate or currently acquired unit contact. Firing spends ammunition, heat and recycle even on a miss or
an out-of-range attempt. Your cockpit sees the target number, roll and outcome.
Glancing hits use the configured rule. Failed saves discard the shot and its
messages together. `fire` and `weapons` take no switches.

Stun, temporarily disabled weapons, recycling, prone support and vehicle cover
can prevent firing before the target is checked. A weapon in the arm carrying a
club cannot fire. Drop the club or use a weapon mounted elsewhere. Ammunition
and ongoing unjamming work are still checked when preparing a shot.

Use `fire <number> <x> <y>` for a coordinate without changing your saved lock.
Conventional weapons select an occupant in that hex; if it is empty, the saved
hex mode controls terrain effects and the hex aim bonus. Artillery always targets
the coordinate and queues its impact. Lua accepts `{x=1, y=2}` as the target in
`btech.unit.fire(unit, pilot, weapon, target)`.

`heat <selection>` (also `flamerheat`) toggles ready flamers between normal damage and heat
transfer. Heat mode adds two heat to the target on a hit, including a glancing hit,
instead of damaging armor. It still costs you three heat and ten seconds of
recycle. `weapons` marks heat mode with `[HEAT]`. Modes survive shutdown and restart.
Select numbers and ranges separated by commas without spaces, such as `0,2-4`.
Reversed ranges work; repeated numbers toggle again. An invalid selection clause
stops processing after earlier changes. Weapons that cannot toggle are skipped
with an error. The command accepts no switches.
For coolant guns, heat mode automatically cools your own unit when `fire` has no
target argument. Supply a target ID or dbref to cool that recipient instead.


Direct fire currently covers Inner Sphere small, medium and large lasers, ER
lasers, pulse and X-pulse lasers, standard/ER/light/heavy/Snub-Nosed PPCs,
machine guns, flamers, SRM-2/4/6, Streak SRM-2/4/6, LRM-5/10/15/20, MRM-10/20/30/40
and AC/2/5/10/20 in supported tactical bipeds. Pulse lasers reduce the target
number by two. MRMs increase it by one and deliver damage in groups of up to five.
Extended-range LRM-5/10/15/20 launchers are also supported. Below ten hexes they
use the lowest two of three attack dice, in addition to the minimum-range penalty.
Their short/medium/long ranges are 12/24/36 hexes.
Dead-fire LR 5/10/15/20 and SR 2/4/6 launchers always use the lowest two of three
attack dice. LR dead-fire missiles deal two damage each at 6/12/18 ranges with a
four-hex minimum; SR dead-fire missiles deal three each at 2/4/6 ranges. Each
missile hits a separate location.
Standard, light and Magshot Gauss rifles are supported. Their ammunition does
not explode, but a critical hit on a functional rifle destroys the entire mount
and causes an internal explosion: 20 damage for standard, 16 for light, and 3
for Magshot. CASE contains transfer from the explosion's section.
Heavy Gauss rifles deal 25 damage through six hexes, 20 through thirteen, and
10 beyond thirteen. Firing while moving forward or backward requires a piloting
check, even when the shot misses. Failure causes a fall. Their eleven-slot
mounts can use linked split slots; weapon criticals cause a 25-damage explosion.
AC/20, LB/20-X and Heavy Gauss split mounts each use one weapon number and recycle timer.
Damage in either part disables the weapon. A split Gauss explosion occurs in
the primary mounting section, even when its extension took the critical hit.
Supported relocated light/XL/XXL engines retain their engine type and mass.
Engine-hit heat and destruction follow the slots actually installed in each
section, so losing a torso with fewer engine slots may leave the unit operational.
ER PPCs have no minimum-range penalty; light and heavy PPCs retain
the standard three-hex minimum. Snub-Nosed PPCs deal ten damage through nine
hexes, eight through thirteen, and five beyond thirteen, before glancing reduction. In-character casualty rules, bridge firing, hex targets, aimed sections, special ammunition and
weapon malfunctions remain unfinished.


Submerged mounts can fire weapons with underwater range bands, including most
lasers and PPCs. Other weapons report `This weapon may not be fired underwater.`
In depth-one water, standing Mechs submerge their leg weapons, including quad
front legs; prone Mechs submerge every mount. Deeper water submerges all mounts.
`sight` and firing use the water ranges, while normal visibility still prevents
shots across a blocked waterline. C3 improves the aiming band without extending
the weapon's physical underwater reach. If range-based energy damage is enabled,
it also uses water ranges.


`inferno <selection>` toggles a ready missile launcher between normal and inferno
ammunition. It requires a matching bin to fire. Surviving inferno missiles cause
burning instead of armor damage: each pair, rounded up, adds three minutes. AMS
can intercept them. One-shot launchers cannot change ammunition modes. `weapons`
marks selected launchers with `[Inferno]`.

Inferno burns reduce cooling by six until their saved timer expires. `status heat`
shows the remaining seconds. Entering deep water or falling prone in shallow water
extinguishes the flames in a cloud of steam. Burning units also light up nearby hexes.

Sustained firing can overheat the reactor. Thermal checks run on saved simulation
timers. At high heat, ammunition can explode before the reactor attempts to shut
down. Your Computer skill can override shutdown; the cockpit shows its target
number and roll. A successful override in an in-character unit awards one
Computer XP when the normal skill cooldown permits it; accepted awards appear
on MechXP. Heat can also injure the pilot, especially with damaged life
support. A reactor shutdown clears your cockpit assignment and target and stops
the unit. If you were moving faster than 10.75 kph, a failed piloting check can
also knock the unit prone. Take the cockpit again and wait for safe startup heat
before restarting. These checks currently cover supported tactical units.

A reactor shutdown during flight causes a fall scaled by remaining jump thrust.
Falling after a jump obstacle or airborne shutdown can also cause a collision
with units crowding the same hex.

An airborne gyro failure can make you fall onto a crowded hex. Losing the last
jump jet also causes a fall, but leaves no thrust for the collision damage rule.


Inner Sphere double heat sinks are supported. Each installed sink occupies three
critical slots and loses two cooling capacity if hit. The unit's listed heat-sink
capacity already includes their doubled cooling. Sink damage persists across
restart; double sinks can coexist with arm-flipping equipment.


Ferro-Fibrous armor (standard, light and heavy) and Endo Steel are supported on
Inner Sphere bipeds. Their material slots do not receive random critical hits.
They reduce calculated mass when enough slots are installed, while armor and
internal damage points continue to work normally.


Standard, light, XL, XXL and compact Inner Sphere fusion engines are supported.
Engine type follows the installed slots. Losing a side torso destroys an XL or
XXL engine; a light engine can survive one lost side torso but produces ten extra
heat while running. A third engine hit destroys any of these engine types.


Installed CASE contains an ammunition explosion within its section. It cannot
save that section or prevent pilot injury, and an XL/XXL unit can still die from
losing side-torso engine slots. Ordinary weapon damage continues to transfer
normally. CASE-II is not yet supported.


Missile attacks must meet their base target number to reach the target or trigger
AMS. When near-miss glancing is enabled, a roll one below that number still
spends the launch but causes no missile damage, pod attachment or Swarm flight.
A roll exactly meeting the base number uses the missile glancing adjustment when
glancing is enabled. Aimed preparation still occurs against an immobile target.

Streak SRM-2/4/6 and Clan Streak LRM-5/10/15/20 launchers require a successful
lock before launching. A failed lock still starts the normal recycle time, but
uses no ammunition or heat.
Successful locks hit with every missile; glancing rules do not reduce the salvo.
Streak LRMs have a six-hex minimum range, cannot fire indirectly or hotload,
and resolve each missile as a separate one-point hit. Their recycle times are 15, 20, 25 and
30 seconds respectively. Angel ECM disrupts their lock protection and makes
them use the Clan LRM cluster tables.


`firecluster <selection>` (also `cluster`) toggles artillery between normal and cluster rounds.
It requires an intact, recycled launcher and preserves an existing smoke or mine
selection by rejecting the change. Matching ammunition is required to fire.

`lbx <selection>` toggles LB-X autocannons between slug and cluster ammunition.
Use a weapon number, comma-separated numbers or a range, as with `flamerheat`.
The assigned conscious pilot must have a running unit and an intact, recycled
weapon. `weapons` marks cluster mode with `[LBX]`. Cluster fire receives a −3
accuracy modifier against VTOLs (including landed ones), or −1 against other
units, and resolves each pellet as one damage point. Slugs deal the
weapon's normal damage in one hit. Only bins matching the selected mode supply
ammunition; an empty selected bin does not switch to the other kind. The selected
mode survives shutdown and restart.

Rocket launchers and other supported one-shot missile mounts carry one salvo
per weapon. `weapons` labels them `[OS]` and shows `spent` after firing. Hits and
misses both consume the salvo; a failed Streak lock preserves it. They do not
use external ammunition bins. Recycling, shutdown and restart do not reload
them. Fire them with the ordinary `fire` command.

Light AC/2, Light AC/5 and the Inner Sphere heavy machine gun support ordinary
fire with their matching ammunition bins. Light autocannons have no minimum
range penalty. Rapid-fire and gatling modes are not yet available.

An installed targeting computer improves eligible direct-fire accuracy by one,
including pulse lasers. It does not assist missiles, flamers, machine guns or
LB-X cluster rounds. Losing or flooding any computer slot disables the bonus
for the whole unit. The bonus applies automatically when firing.

A hardened gyro absorbs its first critical hit without a piloting penalty or
fall check. The second hit damages stability; the third destroys gyro support.
The first hit also preserves an active jump. Hardened gyros weigh twice as much
as standard gyros.

`artemis <selection>` toggles compatible missile ammunition on a recycled
launcher with a working linked Artemis IV controller. Selection accepts weapon
numbers, comma-separated numbers and ranges. One-shot and rocket launchers
cannot change modes. Compatible rounds use their own bins; ordinary ammunition
is not substituted when those bins are empty. Artemis adds two to the missile
hit-table roll, combined with the glancing-hit adjustment and capped at twelve.
Controllers link within their own section. Mechs can also use a head controller
for a center-torso launcher; ground vehicles can use a rear controller for a
turret launcher. VTOL controllers must be in the launcher’s section.

`unjam <selection>` begins a 60-second attempt to clear a jammed ammunition feed.
You must be walking or slower, on the ground, with no weapons recycling. Only
one attempt can run at a time. Firing, jumping and running are blocked during
recovery. A successful piloting check clears the jam and discards one round.
Failure leaves the jam for another attempt. Empty supply clears it without a
check. Shutdown or unconsciousness at completion ends the attempt without
clearing the jam.

`hotload <selection>` toggles hotloading on supported LRM, extended LRM and
long-range dead-fire launchers. It removes the minimum-range penalty, or halves
it when the game option requests that behavior. Cluster hits use the lowest two
of three dice. Attack rolls of two or three jam the feed without spending a shot;
use `unjam` to recover. A hotloaded launcher may explode when critically hit
while ordinary ammunition remains. One-shot launchers cannot change modes.

`ultra <selection>` toggles double-shot firing on supported Ultra autocannons.
Double shots spend two rounds and generate twice the normal heat, with one or
two shells hitting separate locations. With only one round available, the
weapon returns to normal firing. A loader failure in double-shot mode destroys
the weapon; `unjam` cannot restore it.

`rapidfire <selection>` toggles two-round firing on conventional and light
IS autocannons. It spends twice the ammunition and heat, with one or two shells
hitting independently. The weapon returns to normal mode if only one round
remains. A jam requires `unjam`; a catastrophic misload destroys the weapon
and damages internal structure in its mounting section.

`rac <selection> [1|2|4|6]` sets a rotary autocannon's burst length. Omitting
the rate selects one round. Repeating a setting leaves it enabled. Bursts spend
their full ammunition and heat even on a miss; each hitting shell lands
independently. Short ammunition supply resets the weapon to single-shot mode.
Longer bursts jam more easily. Use `unjam` to recover; rotary recovery uses
gunnery with a +3 penalty.

`gattling <selection>` toggles gatling fire on supported machine guns. Each
attempt rolls damage and heat together, and spends three rounds per damage
point. Low supply limits the result; even one or two remaining rounds can fire
for one damage and one heat. Glancing hits reduce damage without reducing heat
or ammunition spent. The command retains the game's `gattling` spelling.

`precision <selection>` toggles Precision ammunition on conventional and light
IS autocannons. It uses separate half-capacity bins and reduces the target's
movement modifier by two, to a minimum of zero. It can be combined with rapid
fire; weapon inspection shows both selections. It does not fall back to normal
ammunition when Precision bins are empty.

`flechette <selection>` selects Flechette rounds on conventional and light IS
autocannons. Against armored BattleMechs, each shell deals half damage, rounded
down. Flechette rounds use separate ordinary-capacity bins and can be combined
with rapid fire. Selecting the mode again returns to normal ammunition.

`armorpiercing <selection>` toggles AP rounds for conventional and light IS
autocannons. AP bins hold half the ordinary rounds. Shots take +1 to their target
number; a hit absorbed by armor can roll for critical damage when the remaining
front or rear armor is below half its original strength. Smaller autocannons
apply a larger penalty to that critical roll. AP can be combined with rapid fire.

`caseless <selection>` toggles caseless ammunition for conventional and light IS
autocannons. Ordinary bins hold twice as many rounds. A firing roll of two or
three jams the feed and checks for propellant ignition, which can destroy the
weapon and cause internal damage. Surviving feed jams can be cleared with `unjam`.

`incendiary <selection>` toggles incendiary ammunition for conventional and light
IS autocannons. Against BattleMechs it deals ordinary shell damage. A critical
hit on a recycling weapon can ignite its loaded rounds if matching ammunition
remains, destroying the mount and causing internal damage.

### Searchlights

On a running unit fitted with `Searchlight`, `slite` starts a five-second on/off switch. Repeating the command preserves its countdown. Lua uses `btech.unit.slite(unit, pilot)`; `btech.unit.state(unit).searchlight` reports hardware and switch state. Switches and damage survive restart.

An intact active lamp illuminates its carrier and targets within 30 hexes in its forward torso arc, subject to terrain, woods and water obstruction. Illumination follows current positions and facing and feeds optical detection and aiming. Front torso hits can destroy the lamp and cancel its switch.

Clan rotary autocannons (`CL.RotaryAC/2`, `/5`, `/10`, `/20`) support the same `rac` burst controls and `unjam` recovery as Inner Sphere rotary mounts. Burst size, remaining ammunition and the attack roll determine ammunition expenditure, heat and jams; each shell that hits applies its weapon's full damage.

Use `mechprefs SLWarn ON` to receive external searchlight entry/exit warnings, `OFF` to silence them, or `mechprefs SLWarn` to toggle. The preference defaults to off and persists with the unit. Lua provides `btech.unit.searchlight_warning(unit, pilot, enabled)`. Multiple beams produce a single illuminated state; warnings resume after restart without repeating an already observed transition.

`mechprefs ArmorWarn` and `mechprefs AmmoWarn` toggle combat warnings; append `ON` or `OFF` to choose explicitly. Both default to on and persist with the unit. Armor warnings report transitions to low, critical or breached protection separately for front and rear armor. Ammunition warnings use installed-bin weighting and the firing mode's warning window, including shots that miss. Lua exposes `btech.unit.armor_warning(unit, pilot, enabled)` and `btech.unit.ammunition_warning(unit, pilot, enabled)`.

`mechprefs FFSafety ON` blocks non-coolant weapon fire at units on your team. Use `OFF` to disable it or omit the setting to toggle; the default is off. A battlefield with the no-friendly-fire flag (256) independently blocks these attacks. Coolant guns can still cool teammates or their own carrier. Lua provides `btech.unit.friendly_fire_safety(unit, pilot, enabled)`.

### Viewing unit markings

From a running cockpit, `view AA` displays the markings of contact AA. `view`
uses your selected unit. The contact must still be visible with unblocked line of sight.
Viewing does not acquire contacts, spend dice or require detailed scan range.
If the target has no description, the reply is `That target has no markings.`

Wizards configure descriptions with `btech.unit.set_markings(actor, unit, text)`;
an empty string clears them. Markings are literal text, persist across restart,
and may contain up to 16,383 bytes. `btech.unit.markings(unit)` reads the raw
configuration. `btech.unit.view(unit, actor, target)` applies cockpit visibility
checks and returns escaped output; omit target to use the selected unit.

In a map room, `view X Y` retains its wizard-only terrain-view behavior.

### Cockpit status

A pilot with the `Sixth_Sense` advantage may receive a private warning when
another unit locks onto their unit. Startup samples the advantage. Successful
checks warn after one to three seconds; the warning gives a general impression
of distance and relative size without identifying the attacker. The pilot must
still be connected and conscious when the warning arrives.

`status` displays the local unit's power, position, movement, heat, armor, internal structure and weapon readiness. Any conscious occupant can inspect it, including while shut down. Select `status armor`, `info`, `weapons`, `heat` or `short`, or combine compact `AIWH` selectors. The short view takes precedence when selected.

The display uses fixed cockpit columns and live ASCII armor diagrams: four
biped silhouettes by weight, a quad, ground vehicles with or without a turret,
and VTOLs. Numbers show remaining protection; colors show damage severity.
Destroyed sections disappear from the outline while the other columns stay in
place. `status R` shows the full display using the model name instead of the
custom display name. `status H` shows only the Mech heat bar.

The weapon block includes limb recovery and installed physical weapons such as
`Axe[RA]`: `Rdy` means the limb is ready, a number counts two-second recovery
ticks, and `XX` means the required limb or actuator is unusable. Attack commands
also check weapon damage and other combat conditions.

Lua uses `btech.unit.status(unit, options)` and receives the same styled text.
Inspection does not advance timers, acquire contacts or consume dice.

`status N` emits the compact chassis record; `status NW` adds weapon locations
and live ammunition columns. Both the regular weapon table and compact export
pair ammunition groups with rows in encounter order, rather than matching them
to that row's weapon. Empty groups are omitted. `S` takes precedence over export
selectors and shows the compact location, heading, speed and condition line.
Info shows coordinates, movement, Mech heat or VTOL fuel, turret facing,
targeting and towing. The header includes power and condition banners.


Use `charge` to select your current target for a ground charge, or `charge #unit`
to select a visible unit. `charge -` cancels. Selection does not start movement:
set your heading and speed to approach the target. A charge is attempted when
movement brings you within collision range. Status shows your charge selection.
With the new charge rules enabled, travel accumulates damage potential and the
selection expires after 61 movement updates. Retargeting keeps accumulated time
and distance; cancelling clears them. Jumping ages the timer without increasing charge distance. An in-range airborne
charge attempt clears the selection without attacking.


Use `jump #unit` for a death-from-above attack, or `jump` with no arguments to
use your current target. You jump toward the hex occupied by the target when
you launch. If it moves out of that hex, you land normally. Status shows the
selected DFA target during flight.

A successful attack damages the target and your legs. A miss damages your rear
armor and leaves you prone, with a chance of pilot injury. Both outcomes impose
physical recovery on your arms, legs and side torsos. An accepted DFA uses this
physical recovery instead of ordinary jump stabilization. `land` can trigger the
attack early if you are already in the target's hex.


### Anti-missile defense

`ams` toggles all installed anti-missile systems on the unit. Defense starts off.
A running unit with defense enabled automatically uses one ready mount against
an incoming missile attack that reaches its base target number. Missed attacks
preserve defensive ammunition, heat and recycle time. Defensive weapons cannot
be fired manually.

Inner Sphere AMS rolls one die for interception; Clan AMS rolls two. A successful
interception reduces the missiles that reach the unit before damage is grouped.
An empty bin prevents activation. Critical destruction disables AMS until its
capability is restored. Lua can toggle with `btech.unit.ams(unit, pilot)` or set
an explicit state with a third boolean argument.

Laser AMS uses the same switch and requires matching ammunition in this game.
Its recycle time is 25 seconds; IS laser AMS generates 12 heat per activation,
and Clan laser AMS generates 1. An installation without a matching bin cannot
intercept missiles.

### Narc beacons

IS and Clan Narc launchers fire homing pods. A pod attaches to a surviving section
without damaging armor or structure. AMS can shoot it down. Destroying the marked
section removes its beacon; otherwise the mark persists across shutdowns and restarts.

`narc <weapon>` toggles Narc-compatible ammunition on a missile launcher. Compatible
missiles gain a cluster bonus against a marked target and require a matching bin.
`explosive <weapon>` toggles explosive ammunition on a Narc launcher: these rounds
inflict damage instead of attaching a beacon. Narc launchers display explosive mode
as `E`, while other launchers display compatible ammunition as `N`.

Lua uses `btech.unit.narc(unit, pilot, weapon)` and
`btech.unit.explosive(unit, pilot, weapon)`. Unit inspection includes `narc_sections`;
normal pod shot reports include `narc`, and explosive rounds use `salvo`.

### Electronic countermeasures

`ecm` and `eccm` select Guardian interference or counter-interference. `angelecm`
and `angeleccm` control Angel suites. Selecting the current mode again switches
that suite off. Guardian and Angel suites operate independently; each requires
working equipment and a running unit. Shutdown and equipment loss disable them.

ECM protects friendly units and disturbs enemies within six hexes. Enemy ECCM
cancels protection, and friendly ECCM cancels disturbance; equal strength cancels.
Angel suites count twice. Team identifiers determine which units are friendly.

Protection on a target or disturbance on a shooter suppresses Narc and Artemis
guidance bonuses. Angel interference also confuses Streak homing: a failed lock
can expend ammunition, and successful attacks use ordinary missile cluster rolls.

Status includes suite selections and current field effects. Lua provides
`btech.unit.ecm(unit, pilot)`, `eccm`, `angelecm`, and `angeleccm`, each returning
`"off"`, `"ecm"`, or `"eccm"`. Unit inspection includes the saved `electronics`
state and its last committed field observation.

### iNarc ammunition

`inarc <weapon> <type>` selects a pod type: `-` for homing, `X` for explosive,
`Y` for haywire, `E` for ECM, or `Z` for Nemesis. Omitting the type selects homing.
Selecting the same type twice keeps it selected. Each type requires matching
ammunition. Lua uses `btech.unit.inarc(unit, pilot, weapon, type)`.

Homing pods improve Narc-compatible missile accuracy and provide the same cluster
bonus as Narc; the bonuses do not stack. Haywire pods impair the marked unit's
weapon accuracy. ECM pods disrupt that unit's electronics. Explosive pods inflict
damage instead of attaching. Nemesis ammunition currently attaches a homing effect.

Several effects can coexist on one section. They persist across restarts and are
removed when the section is destroyed. Unit inspection exposes the section's
`beacons`, and a pod shot report identifies its `kind`.


### Inspecting and removing pods

`pods` shows attached Narc and iNarc effects by location. Use
`removepod <location> <type>` to swat off one iNarc effect: `Y` for haywire,
`E` for ECM, or `-` for homing. Locations include LA, RA, LT, RT, CT, LL, RL and H.
For example, `removepod CT E` attempts to remove an ECM pod from the center torso.

You must be the conscious pilot of a running, placed unit. An arm pod requires
the opposite arm; other locations use your best available arm. The arm must be
intact and free of weapon or physical recovery. Actuator damage makes the attempt
harder. A failed attempt damages the selected location, and either outcome leaves
the arm recovering for 60 seconds. Conventional Narc pods cannot be swatted off.

Lua provides `btech.unit.pods(unit, pilot)` and
`btech.unit.removepod(unit, pilot, location, type)`.

Vehicles use `removepods` to begin a 60-second crew action that removes every
iNarc effect. Conventional Narc pods remain attached. VTOLs must land first.
Stop before starting; the
crew cannot be stunned, unjamming the turret, or unjamming a weapon. Speed commands
and firing are unavailable while the action is pending. The countdown survives
restart and continues after shutdown. Lua uses `btech.unit.removepods(unit, pilot)`;
`btech.unit.state(unit).pod_removal` reports the seconds remaining.

### Stealth armor

`stealth` toggles equipped stealth armor after a 30-second delay. The assigned,
conscious pilot must be in a running, placed unit with working Guardian ECM.
Repeated requests leave the current transition unchanged.

Active armor produces ten additional heat and disrupts your own electronic
guidance. Enemy sensors cannot detect you, so enemies must see you or
probe you with a Bloodhound; your own sensor band and probe are jammed too. Enemies firing at you take range penalties of 3 at medium range, 6 at
long range and 12 at extreme range. Short and minimum range are unchanged.
Shutdown or Guardian damage disables active armor. A pending switch only completes
if the unit is running and Guardian equipment works when the delay expires.

Lua provides `btech.unit.stealth(unit, pilot)`. Unit inspection exposes `stealth`,
including its active selection and any pending switch.

Firing at an active stealth target requires a settled lock on that unit. Missing,
settling or different-target locks reject the shot before ammunition, heat,
recovery or attack dice change. Normal visibility and firing checks still apply.

### Null signature system

`nss` toggles an installed null signature system after a 30-second delay. It
requires the assigned conscious pilot, a running placed unit, and intact devices.
A repeated request does not restart a pending switch.

Active NSS adds ten heat and raises enemy range penalties to 3 at medium range,
6 at long range and 12 at extreme range. Enemy sensors and probes other than
the Bloodhound cannot detect you. It does not create ECM interference or
require enemies to settle a firing lock. If stealth armor is also active, the
range penalty applies once and the two systems' heat adds together.

Shutdown or device loss disables NSS. Pending switches only take effect if power
and devices are available at expiry. Lua uses `btech.unit.nss(unit, pilot)`; unit
inspection includes `null_signature` and its pending destination/countdown.

### Active probes

A working Beagle, Light or Bloodhound Active Probe works automatically out to six,
three or eight hexes; fixed installations reach eight, four and eleven. Probes
see through hills, buildings, woods, smoke and darkness, and they find hidden
units. Aiming through a probe ignores woods and darkness; partial cover still
counts.

A probe contact behind blocking terrain shows a lowercase `p` in `contacts`. You
can lock it, spot it for indirect fire and share it over C3, but you cannot fire
at it directly or scan it until you have a clear line.

Hostile ECM on your unit, or Angel ECM protecting the target, blocks probes. Only
the Bloodhound sees through stealth armor and null signature systems. A damaged
probe stops working; `sensor` shows its condition.

### Radar

Units with AntiAircraft equipment track airborne targets automatically. Radar
needs a target above altitude two and more than one level above its local
surface. Below altitude ten its range is less than altitude squared; higher
targets permit up to 180 hexes. Going beyond the map's ordinary visibility limit
also requires either unit to reach altitude eleven. Radar ignores darkness, smoke,
fire and ECM, but terrain can still block it. VTOL targets and targets at altitude
ten or higher give a three-point aiming bonus; woods and partial cover add
penalties.

Use `tag ID` or `tag #unit` to illuminate an enemy within fifteen hexes with
working TAG hardware and an unobstructed line of sight. Mechs and vehicles
share this control, including TAG built into C3 master computers. TAG takes thirty seconds to settle;
you cannot change it during that timer. Use `tag -` after settling to stop.
Breaking a connection starts thirty seconds of recycling. Moving out of range,
blocked sight, lost sensor contact, shutdown or lost equipment breaks the connection. Another tagger
can take over the same target. TAG state appears in unit inspection.
Lua uses `btech.unit.tag(unit, pilot, target)` and nil to stop.

Use `sguided <weapon>` to select semi-guided rounds on a supported missile
launcher; repeat it for normal rounds. A friendly unit's current TAG removes
positive target-movement aiming penalties. Your own TAG does not provide this
benefit, and losing the friendly TAG removes it. Negative movement modifiers
remain. The launcher needs matching semi-guided ammunition; normal rounds are
not substituted. Lua uses `btech.unit.sguided(unit, pilot, weapon)`.

Use `spot #your-unit` to declare yourself a spotter after weapons and limbs finish
recycling. You cannot fire while spotting; `spot -` ends that role. A friendly unit
can select you with `spot #your-unit` while it has an acquired contact with you.
Selecting another spotter restricts firing to indirect-capable missiles and artillery
until `spot -` clears the link. Lua uses `btech.unit.spot(unit, pilot, observer)`.

For indirect missile fire, select a friendly spotter, clear your own target with
`lock -`, and use `fire <weapon number>`. Your observer must have an acquired unit
target. Its movement, sensor view, spotting skill and settling lock affect aim.
Keeping your own unit target lock uses direct missile fire. Without that lock,
your observer supplies the target even when you give `fire` an explicit target ID.

For artillery, your observer must select a visible hex. The observer must remain
running, friendly, conscious and on your battlefield. Artillery uses the observer's
hex and indirect aim, even if you can also see that hex. Clear any unit target lock
before firing artillery. Retargeting the observer resets accumulated correction.

Use `lock x y` to select coordinates, or append H (hex), B (building), I (ignite),
or C (clear). Coordinate locks settle in eight seconds even outside visibility.
`lock -` clears the selection. With a plain coordinate lock, `fire <weapon>`
attacks its current unit occupant under ordinary firing rules. The lock stays at
the coordinates when that unit moves. Artillery always fires at the selected hex
and arrives after a flight delay, applying its ammunition payload to the area.


Crossing into a mined hex, landing or falling can trigger its mines. Inferno mines
also leave you burning. Vibra mines can detonate nearby when a sufficiently heavy
unit moves through their coverage. Small bomblets spotted on the ground are command
mines; spotting them does not detonate them.


Use `listchannels` (or `listfreqs`) to inspect radio channels. Set a frequency with
`setchannelfreq A=123` (0–999999), a title with `setchanneltitle A=Command`,
and a mode with `setchannelmode A=DuG`. D selects digital, U mutes reception,
and G selects bright green. E enables relay on capable radios and requires D.
An empty title clears it; an empty mode returns to analog. Titles fit fifteen
bytes. The installed radio determines the available channel letters. These
settings are saved across restarts. Use `sendchannel A=Message` to transmit.
Radio works while shut down, but crew stun prevents sending. Analog messages
become scrambled at long range or under ECM; digital messages require range or
friendly relays and are blocked by endpoint ECM. Matching command mines on your
map detonate after the transmission, even if no unit receives the message.


To address a visible acquired contact directly, use `radio #123=Message`, replacing
123 with its dbref, or use its battlefield label such as `radio AB=Message`.
Your unit must be running. A shutdown target receives nothing, and observer units
cannot send targeted radio. This form does not use channel frequencies or trigger
command mines. A recipient that cannot see you receives an unidentified sender name.


## Detailed unit scans

Use `scan AB` (or a unit dbref) to inspect an acquired visible contact. Use `scan`
with no target to inspect your selected unit or coordinate lock. Add `A`, `I`, or `W` for armor,
information or weapons; `scan AB AIW` includes all three. Your unit must be running,
you must be its conscious assigned pilot, and the target must be within scanner
range with clear line of sight. Sensor damage reduces or disables scanner range.

Armor uses condition symbols: `O` above 90%, `o` above 70%, `x` above 45%,
`X` critical, `*` open and `-` destroyed. Weapons show ready, recycling (`-----`)
or damaged (`*****`). Observer units can inspect exact values beyond scanner
range, but still need an acquired visible contact and working scanners.
Use `scan x y` to inspect the first acquired visible unit at those coordinates.
Use `scan x y B` to inspect a building entrance at those coordinates, or
`scan x y H` to inspect buildings and look for mines.

Ordinary scans warn a running target that it is being scanned. The warning names
you only if the target can identify your unit; otherwise it reports `something`.
Observer scans are silent, and shutdown targets receive no warning.

Building scans report current construction factor to your cockpit. Hidden
structures require a successful in-character perception check; invisible buildings
remain undetected. Successful hidden-building detection can earn Perception XP.
Explicit building coordinates must be within scanner range, including for observers.

Mine recognition depends on range and an in-character perception check. Successful
recognition reports bomblets to the cockpit and can earn Perception XP; failed
recognition is reported only to you. Scanning does not detonate or remove mines.

A plain `scan` follows your current lock: building locks report construction
factor, hex locks check buildings and mines, and unit-at-hex, ignition or clearing
locks inspect visible occupants. Scanning does not wait for or advance lock
settling. Observer units may scan selected coordinates beyond hardware range,
but still require visibility and working scanners.


## Brief unit reports

Use `report AB` (or a unit dbref) for a brief, silent view of a visible contact.
It shows identity, position, motion and heat without armor or weapon details.
Use `report x y` for a visible occupant at a coordinate, or `report` for your
current target. Direct unit reports can reach beyond detailed scan range, but
still require a visible acquired contact and working scanners. Coordinate reports
must remain within scanner range. Reports do not alert the target.


## Long-range maps

`lrs` and `lrsmap` invoke the same display. The first letter selects the mode,
so `lrs Terrain`, `lrs Mechs` and `lrs Combined` select T, M and C respectively.

Use `lrs T` for terrain, `lrs E` for elevation/depth, or `lrs M` for
terrain with visible units. Your unit appears as `*`, friendly bipeds as `b`, and
hostile bipeds as `B`. Dark maps mark obscured hexes with `?`.

Add a contact label/dbref to center on it, or a bearing and signed distance to
project the center: `lrs T 180 20`. Negative distance looks behind that bearing.
Display windows stay inside the map. You must be the conscious assigned pilot
of a running unit with working scanners. These displays do not acquire contacts
or alert other units.

Use `lrs C` for terrain-colored elevation, including explicit zeroes. With ANSI
enabled, terrain and unit modes use map colors: your marker is bold, friends are
yellow, enemies and fire are red, woods green and water blue. Ordinary `lrs E`
remains uncolored and leaves zero elevation blank.

Use `lrs L` to filter terrain by current visibility, `lrs H` for filtered
elevation, or `lrs S` for visible units over filtered terrain. These modes work
on ordinary maps as well as dark maps. Question marks mean the requested terrain
information is obscured.

Active fires and inferno-burning units light nearby terrain. Forward searchlights
can reveal terrain farther away at night, but obstructions block their beams.
Visibility-filtered maps update as those light sources move or expire.


`tactical [C|T|B|M|L|U] [target | bearing range]` draws nearby terrain, elevations and
known contacts in a hex grid. Your unit is `**`; friendly contact IDs are lowercase
and enemy IDs uppercase. Use `L` to show only visible terrain, or `U` to show terrain
under fire and smoke. Dark maps always hide unseen terrain. Omit the center to
use your unit, supply a known contact ID, or give a bearing and signed distance.
The display needs a running unit and working tactical sensors. Use `navigate` for a local hex-shaped view.


`tactical C` highlights mech cliffs (three or more elevation levels);
`tactical T` highlights tank cliffs (two or more). Both show your own marker
and elevations, leaving other contacts out of the display. Water and ice depths
count below ground level. Colored displays highlight cliff edges in red; plain
displays use `|`, `!` and `,`. These overlays are unavailable on dark maps.


`tactical B` marks potential landing hexes with O (suitable) or X (unsuitable).
Suitable terrain is grass or road with six equal-height neighboring hexes,
outside landing restrictions that apply to your team. The display uses base
terrain beneath fire and smoke. It shows your own marker but omits other contacts,
and is unavailable on dark maps. This display does not enable aircraft landing.


`tactical M` shows visible minefields as `<>` beneath the terrain/contact cell.
It does not identify mine type, strength, owner or settings, and scripted trigger
fields are not shown. Elevations move to the top of each hex. The display uses
current terrain visibility and does not perform a mine-recognition scan.


`findcenter` reports your current hex and elevation, plus the distance and bearing
from your exact position to the center of that hex. Range is shown to two decimal
places. It works with damaged scanners but requires a running unit. At the exact
center, the bearing readout is 180 degrees.


`navigate [target | bearing range]` combines a local hex map with a compass plot
of positions inside the selected center hex. The compass shows your unit as *,
friendly contacts as x and enemies as X. The readouts always show your own position,
terrain, speed and heading. Omit arguments to center on yourself. This view remains
centered near map edges and can show your own hex with failed scanners.


Tactical and long-range maps use your saved display sizes. Tactical dimensions can
be 5–40 hexes wide and 5–24 high; long-range height can be 10–40 before its odd-row
adjustment. Working sensors and map boundaries can reduce the displayed area.
Navigation keeps its fixed local size. Use `mapdisplay` to configure these preferences.


`mapdisplay` shows your saved sizes. To change them, use
`mapdisplay <tactical-width> <tactical-height> <long-range-height>`—for example,
`mapdisplay 30 20 25`. Use `mapdisplay reset` to restore 21 by 14 tactical maps and
long-range height 11. You can change these settings outside a cockpit.


`contacts +` uses your saved contact categories. Plain `contacts` shows all current
acquired contacts. Saved categories can include or exclude allies, enemies,
shutdown units and wrecks; the selected target can be retained despite those
filters. A target must still be currently visible. Game scripts can configure
saved categories through `btech.player.contact_preferences`.


For a one-time contact filter, use d (wrecks), s (shutdown), e (enemies), a (allies),
and t (selected target). For example, `contacts as` includes shutdown allies.
`!` starts with all categories enabled and excludes the letters that follow:
`contacts !s` excludes shutdown units, while selected-target and wreck settings
remain independent. These options do not change your saved preferences.
Use `b` to include visible building contacts.


`contacts b` shows visible structures and their construction integrity. Combine
b with unit categories to show both, for example `contacts bas`. Concealed buildings
appear only if their identification policy allows it; invisible buildings do not
appear. Status x means restricted identification, X means safe or a restricted
command center, C means an identified command center, and H marks a concealed
building you identified.


`contacts +` also uses your saved building preference. The default excludes
buildings; a saved inclusion preference adds visible structures to the list.
`contacts b` requests buildings regardless of that saved preference.


Use `brief` to see the unit's display settings. `brief C 1` includes buildings by
default; C0, C2 and C3 require an explicit building request. `contacts +` uses
its saved building preference, including the unit mode when set to follow brief.
Use `brief A 6` to disable routine contact notices, or A2, A3 or A5 to hear only
enemy contacts. Weapon-lock loss warnings remain enabled. Settings stay with the
unit across pilot changes and restarts. C0 uses a multiline report with tonnage,
range, speed, heading, coordinates, heat, movement type, arc and condition notices.


Short contact modes (C1, C2 and C3) list structures and wrecks ahead of other units,
with farther contacts first within each group. C0 follows battlefield membership
order. Short lists contain at most 250 entries.


The five characters after `S:` summarize contact condition. They show, in order:
carried club (C); destroyed (D), lamp (L) or illuminated (l); jumping (J), prone (F)
or standing transition (f); shutdown (S), starting (s), excess heat (+) or inferno
(I); and homing beacon (n friendly/N hostile), ECCM (P), ECM (E), protection (p)
or interference (e). A higher-priority condition in a column hides the others.


Contact rows begin with how you currently perceive the contact: `S` sensors, `V`
sight, `R` radar or `P` probe. A lowercase `p` is a probe contact behind blocking
terrain, which you can lock and spot but not fire at directly.


After that letter, `*` means the contact is in your forward torso arc,
`r` right, `l` left, and `v` rear. Buildings use the same symbols. Torso twists
change this indicator. Individual weapons can have different arcs, so the symbol
does not guarantee that every weapon can fire.


Automatic notices use long sentences in A0/A2 and short Seen:/Lost: messages in
A1/A3/A4/A5. Notices include the contact label and arc. A0–A3 color enemy arrivals
red and losses yellow when your terminal supports color; A4/A5 use no color.
A2/A3/A5 show enemies only, and A6 disables routine notices.


A probe contact behind blocking terrain appears as "something". Its condition
columns stay blank, and it is not classified as friendly until identified.


Routine notices normally skip shutdown targets. Use `mechprefs AutoconShutdown ON`
to include them, or OFF to restore the default. This changes notices, not which
contacts appear in your list, and never suppresses weapon-lock loss warnings.


Compact unit rows use battlefield labels such as [ab], followed by B for a biped.
The columns show x/y/z position, r range, b bearing, s speed, h heading, and S status.
Names are shortened to fit the row; friendly labels use lowercase letters.


Building rows show `S` or `V` for how you see the entrance, the arc, the building name, x/y/z,
r range, b bearing, CF current/maximum construction integrity, and S status.
Names are shortened to 23 characters for alignment.


When color is enabled, compact contacts highlight your selected target in red and
other nonfriendly contacts in yellow. Restricted building identification also uses
yellow. Identified allies use the default text color unless selected.

Contact modes C0, C1 and C2 include a list header and footer, even when no contacts
match. C3 displays only contact rows; an empty list produces no output.

Use `lateral ne` or `lateral fr` for Front/Right travel, `nw`/`fl` for Front/Left,
`se`/`rr` for Rear/Right, and `sw`/`rl` for Rear/Left. `lateral -` restores straight
travel. Only quads with all four legs intact can move laterally. A change takes six seconds; requesting
your current direction cancels a pending change. The chassis and weapon facing
stay unchanged. Reverse speed travels opposite the chosen direction. Changes
complete only if the unit is running when the timer expires.
`status info` shows your active lateral direction; heading remains the chassis facing.

Use `bootlegger left` or `bootlegger right` to attempt a 90-degree pivot. You need
at least 43 KPH forward speed and intact legs with no recovering actions or
recycling weapons. Speed, tonnage, damaged leg actuators and submerged water
increase the piloting difficulty. Success halves your current speed and leaves
both legs recovering for 30 seconds. Failure causes a fall whose severity follows
the maneuver difficulty. Your requested speed is retained after a successful turn.

Use `eta x y` to estimate travel time to a coordinate, or `eta` for your selected
ordinary hex. The estimate uses horizontal distance and your current speed,
including reverse speed. It ignores turns, terrain and future speed changes.
A stopped unit reports Never. The result is shared with cockpit occupants.

Use `bearing` for the compass bearing to your selected target, `bearing x y`
for a map coordinate, or `bearing x0 y0 x1 y1` between two coordinates. Unit
targets must still be visible. The answer is private to the requesting occupant.

Use `range`, `range x y`, or `range x0 y0 x1 y1` for distance to a target,
coordinate, or between two coordinates. Spatial distance includes elevation;
ground distance appears alongside it when different. Dark maps hide terrain
height differences from coordinate measurements. Unit targets must be visible.

Use `vector` to combine range and bearing to your selected target. You can also
use `vector x y`, `vector x y z`, `vector x0 y0 x1 y1`, or
`vector x0 y0 z0 x1 y1 z1`. Explicit heights and default targets include a vertical
angle marked + for above or - for below. Vector retains terrain elevations even
on dark maps.

Use `heading` or `speed` without a value to read your current motion.
`speed cruise` equals `speed walk`; `speed flank` equals `speed run`. Numeric
speed requests are limited to your current forward/reverse throttle range.
Movement restrictions such as water and weapon unjamming still apply.

Use `dump all`, `dump <weapon number>`, `dump <section>`, or
`dump <section> <critical slot>` to eject ammunition. Weapon numbers start at zero;
critical slots start at one. For example, `dump RT 1` selects the first right-torso
slot. `dump stop` cancels. Dumping takes time and prevents running and jumping.

`masc` toggles an installed MASC booster. It increases speed but makes progressively
harder failure checks while active. Switch it off to recover between uses. A
failure disables MASC and destroys both hip actuators; moving faster than one MP
also causes a fall. `weapons` and `status` show the current MASC counter.

`scharge` toggles a supercharger. Its overload checks grow harder with use and
recover while switched off. Failure can destroy the engine. Using it with MASC
increases speed further and makes both devices' checks harder. `weapons` and
`status` show the separate counters.


Artillery also supports `hotload <selection>`. Hotloading can jam its loader on a
low firing roll; a jammed launch spends no ammunition or heat. An intact hotloaded
launcher can explode if critically hit while ordinary ammunition remains aboard.
Hotloading leaves the selected artillery payload and its aiming rules unchanged.

Use `stinger <weapon>` to select anti-air Stinger rounds on a supported indirect
missile launcher. Repeat the command to return to normal ammunition. The weapon
must be intact and recycled, and one-shot launchers cannot change modes. Matching
Stinger ammunition is required to fire; ordinary rounds are not substituted.
Stinger shots can engage airborne units but cannot target grounded units or hexes.
They receive a −3 to-hit bonus against flying VTOLs and −1 against units in orbital
descent, including descent controlled by jump jets after a cocoon opens. Jumping
Mechs receive no additional Stinger bonus.
Their maximum reach is seven hexes longer, with ordinary range penalties. Lua uses
`btech.unit.stinger(unit, pilot, weapon)`.

The `stinger` command and Lua operation also work in ground vehicles, including
stationary turrets. Selected ammunition is saved independently of the template.
Mechs and vehicles use the same Stinger targeting and firing rules.

Ground vehicles also accept `lbx`, `sguided`, `precision`, `flechette`,
`armorpiercing`, `caseless` and `incendiary`, with the same Lua operation names.
These change the ammunition selection on compatible, intact, recycled weapons.


Inferno hits on moving-capable ground vehicles follow the configured vehicle fire
rules. Standard rules check for a heat explosion; advanced rules ignite surviving
sections, causing immediate damage and another fire pulse every minute. A fire
can go out after a pulse. Repeated hits do not restart existing section timers.
Stationary installations instead accumulate burning-jelly duration.

Use `shutdown`, then `extinguish` to begin a two-minute crew attempt to put out
section fires. Fires continue to cause damage while the vehicle is shut down.
Lua uses `btech.unit.extinguish(unit, pilot)`; state inspection shows the pending
attempt and section countdowns.


Vehicles can drive through burning terrain. With advanced vehicle fire enabled,
each newly entered burning hex can damage the motive system, sweep the vehicle
with flames, or ignite persistent section fires. Wheeled vehicles and hovercraft
are more vulnerable than tracked vehicles. Staying within one hex does not repeat
the entry check. A disabling result stops the vehicle at that crossing.

## Entering a hangar

At a building entrance, use `enterbase [direction]`. The unit must be running and
moving slowly; Mechs must be standing, and VTOLs must be landed with fuel. An
omitted direction uses the first arrival point. The doors take eighteen seconds to
open. Your current route, movement condition and building lock are checked again
when that delay expires. Moving away or losing eligibility cancels entry.

Entry also follows teleport policies. A damaged building permits forcing its enter
lock only when it is unsafe and below half its maximum integrity.

Use `pickup <target>` to attach tow lines to a visible unit in your hex. Targets
can be given by map label or `#dbref`. Bipeds need both arms and a working
shoulder/hand pair; ground vehicles and VTOLs need salvage equipment. Quads cannot
pick up units. Move at most one KPH horizontally and vertically, and stay within
three levels above or two below the target. Intact hostile units cannot be taken.
Out-of-character targets need scenario towing permission, set by a wizard with
`@btech unit-towable <unit>=on`.

Pickup shuts down and prepares the target, releases anything it was towing, and
applies carried weight to your speed. Pulling a target through ice can break the
surface and affect nearby units. Use `dropoff` to release your tow. A target more
than two levels above its supporting surface falls; closer targets settle.
Reversing while towing requires SalvageTech equipment.

Use `dig` in a stopped tracked or wheeled vehicle to prepare cover over twenty
simulation seconds. Finish turning first. Roads, bridges, buildings, walls and
water cannot be dug into. `status info` shows preparation and completed cover.

Once dug in, only turret weapons can fire and the chassis cannot turn. The lower
sight position can hide the vehicle behind terrain and also restrict its own view. Request
speed greater than 0.1 KPH in either direction to leave cover. A heading change
or movement request cancels preparation; shutdown cancels preparation but keeps
completed cover. Pickup removes cover.

Cover makes the vehicle harder to hit from its elevation or below, according to
the configured cover bonus and front-only setting. Hits are more likely to strike
the turret. These rules apply to attacks from both Mechs and vehicles.

Quad pilots can use `hulldown` to lower the chassis, `hulldown -` to raise it,
and `hulldown stop` to cancel a pending change. Transition time depends on current
maximum chassis speed. Finish standing or jump stabilization first. Lowering
requests a stop. Speed, heading, standing and jumping controls are blocked while
lowered or changing posture.

Hull-down adds protection when terrain already gives partial cover: aiming at a
hull-down unit behind partial cover costs an extra +2, however it is perceived. Shutdown cancels a pending change
and retains the completed posture. Falling, pickup and administrative placement
clear it. `status info` shows the completed stance; `status S` uses `h` for a
pending change and `H` for the completed stance.
Contact posture indicators use `h` while changing and `H` when lowered.

A scenario may mark your unit as fortified. `status info` shows this condition.
It prevents movement, stance changes and towing, and makes the unit count as
immobile for enemy aiming. Shutdown does not remove fortification; a wizard must
change the scenario setting. Weapons and ordinary cockpit inspection remain available.


A running unit with working movement controls can change its desired heading
while falling. Turning does not steer the fall or stop its descent. Ground-vehicle
throttle changes remain unavailable until landing. Quads turn twice as quickly
as bipeds with the same movement capability; the fixed FASA jump-turn rate is
unchanged.


## Weapon groups (TICs)

The assigned pilot can maintain four groups, numbered 0–3, in Mechs and vehicles.
Use `addtic 0 0-2` to add weapon numbers 0 through 2, `deltic 0 1` to remove one,
and `listtic 0` to inspect membership. Comma-separated selections are accepted.
`cleartic 0-3` clears all four groups; `deltic 0` also clears group 0.
Groups survive shutdown, weapon damage and restart. `firetic <groups> [target ID or #unit]`
or `firetic <groups> <x> <y>` fires selected groups in ascending order, then weapons in ascending order. Omit
the target to use the current lock. A rejected shot leaves the remaining weapons
eligible to fire; a fall or shutdown stops the sequence. A weapon in multiple
groups is attempted again, subject to ordinary recycling restrictions. Lua
`btech.unit.tic_fire` accepts the same `{x, y}` target table as `btech.unit.fire`.


## Heat cutoff

`heatcutoff` toggles a Mech's automatic cooling regulator after four seconds.
It needs the conscious assigned pilot; the reactor may be off. Wait for the
current toggle to finish before issuing another. The server may disable new
toggles with `battletech.heatcutoff`.

When enabled, the regulator adjusts active cooling toward the 9–10 heat band.
It changes at most two cooling points per second, or four with double/Clan heat
sinks. It cannot create heat on its own. Turning it off restores disabled
cooling at the same gradual rate while thermal accounting is active. An idle,
stopped Mech finishes the toggle but leaves cooling capacity unchanged until
heat accounting resumes. Water and other environmental effects still
apply. `status` shows the resulting heat production, heat sinks and dissipation.


## Preferred ammunition sections

`usebin <weapon> <section>` prefers matching ammunition in that section.
Use `usebin <weapon> -` to clear the preference. The unit must be on a map and
you must be its conscious assigned pilot. The weapon must be intact and finished
recycling; the reactor may be off. A section does not need a loaded matching bin
when selected.

Shots try the selected section first, then the weapon's section, then other
compatible bins. Empty, damaged or incompatible bins are skipped. The preference
survives shutdown and restart and is shown by `weapons` and `listtic`.
Automatic missile defense retains its own mount-first ammunition selection.

## Self-destruction

Use `explode reactor` or `explode ammo` from a running unit. In-character units
require the assigned pilot, except for wizards; out-of-character units allow
other cockpit occupants. The command requires map placement and completed weapon and physical
recovery. Server settings can disable either request or prevent cancellation.
Ground vehicles and VTOLs normally accept only `explode ammo`, which requires
live destructive ammunition. Gauss ammunition does not qualify.

Engagement releases the pilot assignment. On an in-character unit, a non-wizard
must take the cockpit again before attempting `explode stop`. Shutdown or destruction cancels a pending sequence. Wizards
can append lowercase `override` to use a three-second delay and bypass the
configuration, cooldown and ammunition-safety gates; cockpit authority and the
ammunition-presence check still apply.

The command retains the reference server's timer behavior: with the standard
120-second setting, `explode ammo` advertises ammunition destruction after 60
seconds but a Mech actually detonates its reactor. Vehicles instead lose their
rear section and receive a crew injury. Both requests and all configured timer
boundary cases are preserved; ask the game's administrator about its settings.
Once a sequence is admitted, changing those permission settings does not stop it.


## Dropping prone

Use `prone` in a running biped or quad with its assigned pilot in the cockpit.
The command stops your travel, centers your torso, restores flipped arms and
cancels hull-down posture. You cannot use it while airborne or trying to stand.

At up to one-third of effective maximum speed, the drop requires no control
roll or fall damage. Faster movement requires a control check; above two-thirds
of effective maximum speed the check is two points harder and failure causes a
more severe fall. Reverse travel uses the same absolute-speed thresholds.
Dropping into water can flood exposed compartments and extinguish infernos.
The final ground contact also activates stepping mines.


### Swarm missile ammunition

`fireswarm <weapon>` selects Swarm rounds; `fireswarm1 <weapon>` selects Swarm-1.
Repeat the same command to return to normal rounds. These commands accept the
usual weapon selections and work on Mechs, ground vehicles and VTOLs. A matching
ammunition bin is required to fire. Rockets and dead-fire launchers cannot use
these modes.

After an initial hit, unused missiles can attack nearby units in map order.
Ordinary Swarm can hit friends and return to its launcher. Swarm-1 excludes
friendly secondary targets. Retargeting requires the previous target to see the
next unit within 1.9 hexes. Combat-safe units are skipped as secondary targets.
The flight stops when its missiles are spent, its cumulative range runs out, or
it has attacked eleven distinct targets. AMS does not intercept these rounds.
An initial miss ends the flight; secondary misses retain the unused missiles.
Empty-coordinate fire does not retarget.

Lua provides `btech.unit.fireswarm(unit, pilot, weapon)` and
`btech.unit.fireswarm1(unit, pilot, weapon)`. A Swarm firing result contains
`salvo.kind = "swarm"` and ordered attacks in `salvo.report.hops`.


### Smoke and mine missile modes

`firesmoke <selection>` and `firemine <selection>` select Smoke or Mine ammunition
on missile launchers. Repeat the selected command to return to normal ammunition.
Matching supplies are required to fire. Both controls work on Mechs and vehicles;
rockets, one-shot launchers and artillery are rejected.

These missile modes retain ordinary missile damage. Mine rounds bypass AMS;
Smoke rounds can be intercepted. They do not create smoke clouds or minefields.
Artillery templates can separately carry Smoke and Mine payloads, which create
their environmental effects on arrival. These cockpit controls follow the
reference's missile eligibility rather than selecting those artillery payloads.

Lua provides `btech.unit.firesmoke`, `btech.unit.firemine` and
`btech.unit.firecluster`; `btech.unit.cluster` remains available.

Use `target <section>` to aim at a part of your selected unit target, or
`target -` to turn anatomical targeting off. This requires the conscious assigned
pilot of a running unit. Section names follow the target's anatomy, including
quad front legs and vehicle sides. Ground vehicles have no rotor selection.
Your preference stays selected through lock changes and shutdown; clear it when
you no longer want it.

An immobile target gives an ordinary direct shot a chance to hit the selected
exposed section. Eligible targeting-computer weapons can also direct hits, with
a +3 accuracy modifier against mobile targets instead of their usual -1 bonus.
Head selection against a Mech costs +7 when immobile or +25 when mobile and
replaces the computer bonus. Missile and shotgun clusters do not direct their
hit locations. Hidden sides and covered legs fall back to ordinary hit routing.

Use `sight <number>` to check the selected target, or add a contact ID, `#unit`,
or `x y` coordinates just as with `fire`. Sighting displays the base-to-hit
number or reports that the target is out of range. It works while the weapon
is recycling or out of ammunition, and preserves hiding and weapons hold.
It consumes the aiming dice but never fires, spends ammunition, adds heat,
or clears a jam. Destroyed and defensive-only weapons cannot be sighted.

`weaponstatus` lists every installed weapon's component condition and damage
penalties, including preferred ammunition sources and damaged, destroyed or
disabled slots. A conscious passenger may use it while the unit is on a map,
including while shut down. Empty ammunition and recycling belong to the
`weapons` readiness report and do not imply equipment damage.

`weaponspecs` lists one row per installed weapon type: heat, damage, range bands
and recycle time. It includes destroyed installations and works before the unit
is placed on a map. The extreme-range column appears when the server enables
extended ranges; artillery maximum ranges are shown in hexes.

`critstatus <section>` shows every equipment slot in the selected section, including
empty slots. Use a section abbreviation such as `la`, `ct` or `h`; quads use
`fll` and `frl` for their front legs, and vehicles accept names such as `front`,
`turret` or `rotor` when those sections exist. The report shows broken, destroyed,
disabled and damaged equipment, ammunition quantities and capacities, spent
one-shot launchers, rear mounts and Artemis links. Split slots identify their
linked weapon. You must be the conscious assigned pilot, but the unit may be
shut down or off-map. Manufacturer labels follow the server's parts setting.

`chop [left|right|both] [#unit]` uses the same sword attack as `sword`, including
targeting, damage and recovery.

`disable <weapon>` powers down a Gauss weapon once it finishes recharging. You
must be piloting a running unit on a map. Comma-separated weapon numbers and
ranges are accepted. A powered-down weapon cannot fire and will not explode
when hit critically. It remains installed, keeps its ammunition, and stays
powered down after shutdown or restart. There is no cockpit command to power it
back up.

Clan ATM-3/6/9/12 launchers use 5/10/15 range bands, a four-hex minimum range,
and two damage per missile. `atmrange <selection>` toggles Extended Range bins;
`atmexplosive <selection>` toggles High Explosive bins. These commands also work
on other eligible indirect missile launchers. A matching bin is required; toggle the command again to return to normal
ammunition.
As in the reference game, these two ammunition labels do not alter range or
damage. Templates use the `ExtendedRange` and `HighExplosive` flags.

MML-3/5/7/9 launchers use dedicated MML ammunition. `mml <selection>` toggles
between SRM and LRM supplies. SRMs use 3/6/9 range bands, no minimum range, and
individual two-point hits. LRMs use 7/14/21 bands, a six-hex minimum, and one
point per missile grouped into at most five-point hits. Both use cluster rolls
and can be intercepted by AMS. Only the LRM family can use a spotter.

Normal MML bins contain SRMs; `MML_LRM` marks LRM bins and initial launcher
selection. For sizes 3/5/7/9 respectively, a full ton holds 33/20/14/11 SRM
salvos or 40/24/17/13 LRM salvos. Half-ton capacity rounds down after choosing
the family. Missing selected ammunition rejects firing; it never switches
families automatically. SRM Artemis, Narc and Inferno modes use the existing
controls; combined LRM special-ammunition flags are not supported.

`weaponspecs` lists both MML ammunition profiles, labelled SRM and LRM.

`safety [on|off]` reads or sets MechWarrior safety. You must be the assigned
pilot in the cockpit. `mechprefs MWSafety ON|OFF` changes the same setting.
Completed startup turns safety on; shutdown and interrupted startup retain its
setting. This preference concerns MechWarrior targets, which are not currently
supported combat targets. It does not change friendly-fire safety.

`ap <weapons>` is an alias for `armorpiercing <weapons>`. Both use the same
armor-piercing ammunition selection, including comma-separated weapon numbers
and ranges.
