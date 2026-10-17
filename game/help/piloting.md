+++
title = "Piloting BattleMechs"
description = "Enter a unit and take or release its cockpit"
keywords = ["status", "view", "markings", "ap", "safety", "mwsafety", "mml", "explode", "self-destruct", "usebin", "heatcutoff", "addtic", "deltic", "cleartic", "listtic", "firetic", "hulldown", "dig", "pickup", "dropoff", "enterbase", "pilot", "unpilot", "piloting", "cockpit", "startup", "shutdown", "heading", "speed", "rottorso", "fliparms", "slite", "searchlight", "sensor", "contacts", "lock", "stand", "prone", "lrs", "lrsmap", "fire", "sight", "target", "weapons", "weaponstatus", "weaponspecs", "critstatus", "flamerheat", "heat", "inferno", "lbx", "cluster", "firecluster", "firehoming", "firesmoke", "firemine", "fireswarm", "fireswarm1", "artemis", "apollo", "unjam", "hotload", "ultra", "rapidfire", "rac", "gattling", "armorpiercing", "caseless", "precision", "flechette", "jump", "dfa", "death from above", "push", "ram", "land", "unitprefs", "autofall", "ams", "a-pod", "b-pod", "talons", "spikes", "torso cockpit", "mga", "machine gun array", "pods", "removepod", "removepods", "melee", "axe", "sword", "mace", "saw", "claw", "lance", "flail", "wrecking ball", "chain whip", "vibroblade", "retractable blade", "magma", "lava", "wind", "gravity", "temperature", "emi", "electromagnetic interference"]
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
startup, and releases the cockpit. Use `pilot` again before restarting. A Mech or vehicle
shuts down only once it has come to a stop on the ground: `shutdown` is refused while it
has any forward or reverse speed or is in the air, so stop, or land a VTOL, first.
Shutting down takes no piloting roll and never tumbles a vehicle (Total Warfare p. 160).
A reactor cannot restart at 30 or more heat, the operator override included.
From 14 to 29 heat, `startup` rolls 2d6 against the highest current shutdown
avoid number (4 at 14, 6 at 18, 8 at 22, 10 at 26); a failed roll keeps the
reactor down until the next 30-second turn (Total Warfare p. 160).

Startup progress survives restart. It pauses while the server is offline or
unable to save progress. A running unit accepts `heading <degrees>` and
`speed <kph|stop|walk|run|back>`. Walking speed is your template's walking MP at 10.75
kph per point and running is walking x 1.5 rounded up (Total Warfare p. 53); you
may back up no faster than walking. Heat takes one walking MP per five points and
caps the throttle rather than scaling it, hot triple-strength myomer adds two walking
MP, and MASC doubles the walking figure that is left, running refigured each time
(TW pp. 137, 139, 159). A Mech that has lost a leg walks at 1 MP whatever would raise
it, myomer and MASC included, and cannot run (TW p. 122). Away from standard gravity
your walking MP is divided by the G-rating and rounded to the nearest whole MP, halves
down, after every other modifier, and running is figured afresh from it; a unit left
with no MP cannot move (Tactical Operations: Advanced Rules p. 53). A Mech that runs
during a thirty-second turn with a damaged hip or gyro makes one piloting roll at the
turn's end (TW p. 53), and one that exceeds its normal running MP under light gravity
rolls once a turn against a point of leg damage per MP over; a vehicle that exceeds its
normal flank speed rolls the same way against a point of front internal damage per MP
over and a motive damage roll (Tactical Operations p. 53). Being shut down or having
an unconscious MechWarrior forces no roll of its own. Turns and speed changes are
gradual. Each hex costs movement for its ground, foliage, route, water and any ice,
snow or mud, and your speed is divided by that cost, following the Total Warfare and
Tactical Operations movement cost tables. A road or rail line replaces the
ground and foliage it crosses. Rough ground, rubble, light woods and rail cost one
extra; ultra rough ground, ultra rubble and heavy woods two; ultra-heavy woods three.
Jungle costs one more than woods of the same density. Swamp costs Mechs one extra,
hovercraft nothing, and other vehicles two. Magma costs one extra, and heavy industrial
ground one extra for Mechs. Sand and thin snow slow only wheeled
vehicles. Deep snow, mud and ice cost one extra, except for hovercraft, and ice costs
that over water as on land. Water one level deep costs one extra and deeper water
three; rapids add one more and torrents two. Fire slows every ground unit at least as
much as rough ground; smoke does not. Standing in flames at the turn boundary adds five
outside heat to a Mech, counted against the fifteen-point outside-heat cap, unless it is
under the water or beneath the bridge deck the fire burns on.
Each level climbed or descended to enter a hex costs one MP (Total Warfare p. 52),
charged like the hex's own terrain cost: you keep your throttle and cross the hex
that many hex-widths slower. Backing across a step also requires a piloting check (a
house rule; the book forbids backward level changes): failure causes a fall, with an
uphill fall returning you to the previous position. Map edges and unresolved altitude
transitions stop the unit. Buildings and walls use their
mapped heights.

Some terrain cannot be entered at all, and a unit that reaches its edge simply
stops there. Mechs may change at most two levels between hexes and vehicles one; a
steeper slope is impassable. Mechs cannot push into ultra-heavy woods or jungle.
Wheeled vehicles keep out of rough ground, rubble, ultra rubble, woods, deep snow and
magma crust; no vehicle enters heavy or ultra-heavy woods, any jungle, ultra rough ground
or liquid magma; hovercraft also avoid light woods; and nothing but a hovercraft or an
amphibious vehicle crosses water a level or more deep. Both ride the surface rather
than the bed: a hovercraft skims water for its ordinary cost, and an amphibious
vehicle pays two points for every water hex whatever its depth (Total Warfare p. 52),
so neither is ever under water. A road or bridge carries a
unit through terrain it could not otherwise enter, and buildings and walls are
climbed onto rather than entered.

Running or flanking across pavement, a road or a bridge deck, or crossing ice at
any speed, risks a skid: change facing on such a hex and then move on, and you
roll against your piloting or driving skill, harder the farther you have moved
this turn. Failure drops a Mech and slides it along its old course for half the
hexes it had moved, taking half a fall's damage in every hex it slides through,
stopping against higher ground and tumbling over any drop too steep to walk
down. Every hex the skid crosses uses up the movement points its terrain costs, so
woods or rough ground cut it short; a building the skid brings down lets it go on,
less the building's movement cost (Total Warfare pp. 62-63). A skidding unit that reaches another unit charges it with the +3 modifier
for an unintentional charge (Total Warfare p. 63); a hit ends the skid, a miss
slides on through. A vehicle slides the same way and rolls once for motive
system damage, and a vehicle that skids into terrain its drive could never
enter takes a point of damage per five tons and is immobile there for good. A
hovercraft does not skid but sideslips: whenever it changes facing at flank
speed on any ground and then moves on, it rolls against its driving skill, and
on a failure it drifts along its old course one hex for every point it missed
by, without damage unless it drifts into something or off a drop deeper than one
level, which is a fall.

Swamp, mud, deep snow, tundra and sand can bog you down. Entering such a hex
takes a piloting or driving roll, one easier on tundra, deep snow and mud, and a
Mech that falls in one, or jumps into a swamp, is stuck at once. A stuck unit
cannot move or turn, is two easier to hit, and rolls again to pull free at the
start of every turn. Jumping out always works. Hovercraft never bog down.

Stacking follows Total Warfare p. 57: a hex may hold at most two units a side and
four in all, only one of them a Mech. You may drive or walk through a hex your
friends hold, but you cannot stop in one that is already full, and you stop at the
edge of any hex an enemy holds (a charge at that enemy goes through). You may not aim
a jump at a full hex. A Mech brought down out of the air into a hex another Mech holds
falls no levels, so it shoves that Mech out of the hex in the direction it was
travelling, and the shoved Mech must make a piloting roll or fall (the domino effect,
TW pp. 152-153). A Mech whose jump lands in a hex that filled while it flew makes an
accidental fall from above (TW p. 152): roll 7 or better, modified for the target's
movement and cover, to come down on it for your tonnage over ten per level fallen, then
take your own fall onto your back; a miss leaves everyone standing.

Water depth affects piloting checks. Entering water a level or more deep requires a
piloting check, one easier at depth one, unmodified at depth two and one harder
beyond; slipping causes a fall. A running Mech stops at the water's edge, and
entering limits forward throttle to walking speed. Water can flood breached
sections, disable equipment and cause loss of balance: a location under water
floods once its armor is gone, and every hit that damages it, falls included,
breaches it on 10 or more on 2d6 (Total Warfare p. 121). A fall into water from
above its surface takes half damage for striking the surface and half again for
reaching the bottom (p. 57). With life support destroyed, a pilot whose Mech is
submerged takes a point of damage at the end of every turn (p. 127). Running
commands are rejected while in the water.
Bridge travel selects the deck or a lower surface at elevation -1 according to
your current height. Beneath a bridge you stand on the river bed: water checks
still apply, and the deck overhead plays no part in slope checks.

Stepping onto ice takes no roll: the extra movement point it costs is the price of
moving carefully across it (Tactical Operations: Advanced Rules pp. 48, 63). Each new
hex of frozen water entered from
elevation zero also has a one-in-six chance of breaking, and landing a jump on it
breaks it half the time. A break can drop everyone on that hex into the water.
Below intact ice, you follow the bottom and make water checks; falls and standing
keep you below it. Moving from elevation -1 into depth-one ice brings you onto its
surface without a fracture roll on that entry.

Terrain and weather make every piloting check you take there harder. Sand, tundra,
magma crust, heavy industrial ground, deep snow and mud each add one, and so does thin
snow for wheeled vehicles. Light, heavy and ultra-heavy jungle add one, two and three,
and swamp adds one for Mechs and two for tracked and wheeled vehicles. Ice adds four,
rapids two, torrents three and liquid magma four. Hovercraft ignore the swamp, ice, snow,
mud and fast-water penalties. A strong gale adds one for Mechs and two for hovercraft and
VTOLs, a storm three, a tornado three for everyone and an F4 tornado five. A Mech
entering rubble takes a piloting check, at +1 for ultra rubble; failure is a fall.

Magma crust can crack under you: each crust hex you walk or drive into breaks into
liquid magma on a one-in-six chance, and landing a jump on it breaks it half the time.
Crust adds five to a Mech's heat each turn it stands there and two more as it steps on,
liquid magma ten and five, and unlike flames that heat has no cap. Liquid magma burns a
Mech's legs for 2D6 each when it enters, and again at the start
of every turn it stays there; a fallen Mech burns everywhere. Any vehicle that enters liquid
magma is destroyed. Deep snow draws one point of heat a turn from a Mech with heat sinks
in its legs. Beyond -30 or 50 degrees, vehicles lose a cruising MP for every started ten
degrees.

Wind and gravity also affect weapon fire. A moderate gale adds one to missile fire; a
strong gale two to missiles and one to ballistic weapons; a storm three and two. In a
tornado missiles cannot fire, ballistic weapons take three and energy weapons two; an F4
tornado leaves only energy weapons, at three. Missiles and ballistic weapons also take one
for every full 0.2 G the gravity differs from normal.

Electromagnetic interference, when a map has it, adds two to every ranged weapon attack
and takes two from every roll on the Cluster Hits Table (missiles, LB-X cluster rounds,
machine gun arrays, rotary and Ultra autocannon bursts and hyper-assault Gauss rifles),
never below 2; Lua's aim breakdowns report the +2 as their `interference` term. It also
renders active probes useless and doubles the reach of ECM suites to twelve hexes. Physical
attacks and the built-in sensors are unaffected (Tactical Operations: Advanced Rules p. 53).

Use `unitprefs` to inspect AutoFall. `unitprefs AutoFall ON` and `OFF` set the
preference and `unitprefs AutoFall` toggles it. The setting stays with the unit
through shutdown and restart, and the assigned pilot can change it with the engine
off, but it changes nothing in play: a drop too steep to walk down cannot be
entered, so there is no cliff to fall off. Only AutoFall is supported by `unitprefs`.

`pilot`, `unpilot` and `shutdown` take no arguments; these commands take no switches.

An unconscious pilot cannot take a cockpit or operate its controls. Recovery
attempts occur every 30 simulation seconds, and the countdown survives restart.
Releasing the cockpit does not clear unconsciousness. Piloting checks fail
without rolling while the pilot is unconscious; an already prone Mech retains
its automatic-success exception.

Use `jump <bearing> <range>` to engage jump jets toward a hex center. Grassland, roads,
forests, rough ground, mountains, snow, sand, smoke, fire, water, ice, bridges,
buildings and walls are supported,
including changes in elevation. Jump progress survives restart
and pauses while the server is offline or unable to save. You have one jump MP
per working jet; in unusual gravity your jump MP is divided by the G-rating and
rounded to the nearest whole number, halves down. Range is limited to your jump
MP, and so is the climb to the landing hex, but you may jump down any number of
levels. A jump may not cross a hex whose surface (ground, roof or bridge deck) is
higher than your jump MP plus the level you took off from; such a jump is refused
before you leave the ground, so a jump that launches never hits terrain. A range of
zero jumps in place and lands in your own hex. Jumping builds up one heat per hex
jumped, at least three, charged at takeoff; improved jump jets charge one per two
hexes, still at least three. Gravity does not change the heat.
Jumping and death-from-above attacks are refused when load-adjusted effective
speed is more than one MP (10.75 KPH) below the unloaded maximum. CargoTech,
movement boosters, TSM and special gravity affect that calculation. A loss of
exactly one MP is allowed. You cannot jump while towing another unit.
Wait twelve seconds after an ordinary landing before jumping again. Stabilizing adds two
to firing difficulty. Damaged landing gear or loss of consciousness can cause
a fall, and losing all jump thrust or overheating can cut the flight short; a
fall in flight is a zero-level fall. A quad missing one leg lands at +5 and has
lost its -2 piloting bonus. On a low-gravity world, jumping more hexes than your
normal jump MP strains the legs: a piloting roll at +1 per full half G below
standard, and failure costs each leg one point of internal structure per hex
beyond your normal jump MP.
Use `land` to attempt an early landing at your current location. A failed
control check causes a fall; a successful abort still requires any damaged-leg
or gyro landing check. Ground speed changes are unavailable in flight. Use `heading <degrees>` to turn
while jumping; facing changes do not steer the jump away from its landing hex.
You cannot `shutdown` during a jump; land first. A Mech that loses power in the air
some other way, such as a carrier releasing it aloft or an administrative shutdown,
stops its horizontal travel and starts free fall. It drops every three seconds,
accelerating until impact. Restarting the engine does not arrest this fall. After
restarting, you can attempt `land`, but the pending fall event still causes an
impact when due.
You can fire during a jump: jumping adds three to your firing difficulty, and
a jumping target adds one plus its current jump-speed movement modifier.
Losing both legs or hips leaves you unable to stand while your jets finish the
jump; further jet loss can still bring you down.
Crossing ice during a jump can break it and drop other units standing on that hex.
Breaking downward can end your jump in a fall; breaking upward preserves your flight.
Water crossings use your current altitude. Submerging can flood breached sections
and improve cooling; takeoff is limited to water one level deep, and from water one
level deep only torso-mounted jets can fire. Landing in water a
level or more deep takes a piloting check; failure tumbles you to the bottom for
a fall as many levels as the water is deep, at half damage. Intact ice is at
surface level. Landing on it breaks it half the time, dropping units on that tile
into the water, and ice that holds takes a piloting check to keep your feet.
Landing in a swamp sticks you fast, and landing in liquid magma or other soft
ground rolls against getting stuck. DFA remains unavailable.
Accumulated damage never adds a takeoff or landing check. Twenty or more points of
damage in one turn force the turn's single piloting check, which can cause a fall
in flight.

Cockpit stun blocks firing for ten simulation seconds and slows a running unit
toward walking speed. Another stunning hit restarts that countdown.

Use `rottorso <left|right|center>` (or `l`, `r`, `c`) to turn the torso one
hexside (60 degrees). Torso, head and arm weapons turn with it; leg weapons follow
the body heading. Turning in the opposite direction first returns it to center.
`fliparms` toggles arm facing on chassis that support it: reversed arms fire only
into the rear arc. A Mech cannot twist its torso and reverse its arms at the same
time, so center the torso before `fliparms` and flip the arms forward before
`rottorso` (Total Warfare p. 105). Both require a running, standing unit and its
conscious pilot; quads cannot twist. Facing survives restart. These commands take
no switches, and `fliparms` takes no arguments.

Perception is automatic; there is nothing to switch. `sensor` shows how far and
by what means your unit can perceive right now:

- Sight: the light and the weather set how far you see; at night a lit target
  can be seen out to forty-five hexes and your own searchlight lets you see
  thirty. Darkness also costs to hit; see `help night`.
- Sensors: your infrared, magnetic anomaly, radar and seismic sensors. Every
  thirty seconds each unit rolls one sweep, and a target sight misses is found
  if any system's range band for that roll covers it; the report lists each
  system's brackets and the current roll and band.
- Probe: a carried active probe, which sweeps ahead of the built-in sensors
  with longer brackets, and lets your attacks see a little way into nearby woods.

See `help line of sight` for the full rules. `sensor` takes no arguments. Lua
`btech.unit.perception(unit)` returns the same report as a table.

`contacts` lists acquired targets that are currently visible, nearest first, with
unit number, chassis, friendly/hostile relation, range, bearing, heading and speed.
`contacts #unit` limits the display to one acquired target. Conscious occupants can
use this display while the unit is running. It does not re-sweep sensors or
reveal positions from stale observations. `contacts` takes no switches.

Use `lock #unit` to select a currently visible contact from `contacts`, or `lock -`
to clear your target. Selection requires the running unit's conscious assigned
pilot and takes effect at once. Your lock is your primary target and takes no
to-hit modifier; firing at any other unit is a secondary attack at +1. Without
a lock every shot is a secondary attack at +1. A scanner update reporting your
selected contact lost clears the lock. Shutdown and battlefield removal clear
selection. `lock` takes no switches. Aimed-section commands are not yet available.

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
get up. An attempt costs 2 MP (Total Warfare p. 50): at your running pace, a hex every
sixty seconds per MP, it takes 120 seconds divided by your running MP, never more than
a thirty-second turn. `stand careful` spends the whole thirty-second turn standing, for
a -2 modifier if you have more than two walking MP (Tactical Operations: Advanced Rules
pp. 21-22). If your base target exceeds twelve, use `stand anyway` to
attempt it despite certain failure. Every attempt adds one point of heat (Total Warfare
p. 159). Failed attempts cause another fall. Successful attempts make you upright but
block forward or reverse movement until the stand timer finishes. You can use `heading`
to turn while prone, getting up or recovering from a failed attempt. A Mech can stand
missing one leg, one arm and one leg, or both arms, but not one leg and both arms, and
not without walking MP (TW pp. 50-51); it can stand on land or in water.
Ice and bridge standing still require their terrain rules. `stand` takes no switches.


An operator can place your unit in weapons hold. While held, `fire` and
`firetic` are rejected; `status info` shows `WEAPONS HOLD`.
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

Weapon range is the number of hexes from your hex to the target's, counting the
target's hex (Total Warfare p. 109). Height never adds to it: a VTOL hovering ten
levels above an adjacent unit fires at range 1, minimum-range penalty and all.
Range brackets, minimum range, maximum reach, C3 sharing and artillery reach all
count hexes this way, and the `r:` column of `contacts` shows that count.

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
transfer. Heat mode adds two outside heat to a Mech target on a hit, including a
glancing hit, instead of damaging armor; outside heat from flamers, infernos,
plasma and fire together cannot exceed fifteen points a turn (Total Warfare
p. 159). A vehicle tracks no heat, so a heat-mode hit on one does its normal
damage instead (Total Warfare p. 113). It still costs you three heat and ten seconds of
recycle. `weapons` marks heat mode with `[HEAT]`. Modes survive shutdown and restart.

Select numbers and ranges separated by commas without spaces, such as `0,2-4`.
Reversed ranges work; repeated numbers toggle again. An invalid selection clause
stops processing after earlier changes. Weapons that cannot toggle are skipped
with an error. The command accepts no switches.

A Clan plasma cannon never damages a Mech's armor: a hit adds 2D6 outside heat
instead, under the same fifteen-point cap. Against a vehicle it deals 3D6 damage
in five-point groups (Total Warfare p. 139). A Clan hyper-assault Gauss rifle
rolls on the Cluster Hits Table for its size, two higher at short range and two
lower at long range, and lands its damage in five-point groups (Total Warfare
p. 136). A critical hit on a hyper-assault Gauss rifle explodes for 10, 15 or 20
points by size, and on a Clan anti-personnel Gauss rifle for 3 (p. 135).


Direct fire currently covers Inner Sphere small, medium and large lasers, ER
lasers, pulse and X-pulse lasers, standard/ER/light/heavy/Snub-Nosed PPCs,
machine guns, flamers, SRM-2/4/6, Streak SRM-2/4/6, LRM-5/10/15/20, MRM-10/20/30/40
and AC/2/5/10/20 in supported tactical bipeds. Pulse lasers reduce the target
number by two. MRMs increase it by one and deliver damage in groups of up to five.
Extended-range LRM-5/10/15/20 launchers are also supported. Their
short/medium/long ranges are 12/24/36 hexes.
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
lasers and PPCs, and only at targets in the water. Other weapons report `This weapon
may not be fired underwater.` In depth-one water, standing Mechs submerge their leg
weapons, including quad front legs, while their arm, torso and head weapons fire
normally at targets above the surface; prone Mechs submerge every mount. Deeper
water submerges all mounts. Standing in water adds nothing to your own shots.
`sight` and firing use the water ranges, while normal visibility still prevents
shots across a blocked waterline. C3 improves the aiming band without extending
the weapon's physical underwater reach. If range-based energy damage is enabled,
it also uses water ranges.


`inferno <selection>` toggles a ready missile launcher between normal and inferno
ammunition. Inferno rounds are an SRM munition (Total Warfare p. 141): only standard
SRM launchers and MMLs loaded with SRM rounds carry them; LRM, Streak, MRM, rocket,
ATM, Thunderbolt and Narc launchers do not. A matching bin is required to
fire. Surviving inferno missiles add
two heat points each to a Mech instead of armor damage, as in Total Warfare;
outside heat is capped at fifteen points per turn. AMS can intercept them.
One-shot launchers cannot change ammunition modes. `weapons` marks selected
launchers with `[Inferno]`.

Sustained firing can overheat the reactor. Thermal checks run on saved simulation
timers using the Total Warfare heat scale: at high heat, ammunition can explode
before the reactor attempts to shut down, and each check is a 2d6 avoid roll
whose target rises with heat. Shutdown at 30 heat cannot be avoided. Carrying
inferno ammunition adds its own, earlier ammunition avoid rolls; laser heat sinks
add one to both kinds of ammunition roll. Heat can also injure the pilot once life
support is destroyed. A heat shutdown clears your
target and stops the unit, but you keep the cockpit. The involuntary shutdown
forces a piloting check at +3 whatever your speed, and failing it knocks the unit
prone (Total Warfare p. 60). The reactor restarts by itself at the next turn
boundary once heat falls below 14, even if you are unconscious; from 14 to 29
`startup` needs the avoid roll described above, and at 30 or more nothing
restarts it (Total Warfare p. 160). These checks currently cover supported
tactical units.

A reactor shutdown during flight causes a zero-level fall. Falling after an
airborne shutdown can also cause a collision with units crowding the same hex.

An airborne gyro failure can make you fall onto a crowded hex. Losing the last
jump jet also causes a fall, but leaves no thrust for the collision damage rule.


Inner Sphere double heat sinks are supported. Each installed sink occupies three
critical slots and loses two cooling capacity if hit. Templates and the template
report count heat sinks as the record sheet does, one per sink; each double sink
dissipates two heat. Sink damage persists across
restart; double sinks can coexist with arm-flipping equipment.


Ferro-Fibrous armor (standard, light and heavy) and Endo Steel are supported on
Inner Sphere bipeds. Their material slots do not receive random critical hits.
They reduce calculated mass when enough slots are installed, while armor and
internal damage points continue to work normally.


Standard, light, XL, XXL and compact Inner Sphere fusion engines are supported.
Engine type follows the installed slots. Losing a side torso destroys an XL or
XXL engine; a light engine can survive one lost side torso but produces ten extra
heat while running. A third engine hit destroys any of these engine types. An XXL
engine runs hotter: two heat a turn standing still, four walking and six running,
and double heat per hex jumped with at least six a jump (Tactical Operations:
Advanced Units & Equipment p. 121).


Installed CASE contains an ammunition explosion within its section. It cannot
save that section or prevent pilot injury, and an XL/XXL unit can still die from
losing side-torso engine slots. Ordinary weapon damage continues to transfer
normally. CASE-II is not yet supported.


Missile attacks must meet their base target number to reach the target or trigger
AMS. A roll exactly meeting the target number is a glancing blow (Tactical
Operations): direct damage is halved, rounding down, and a missile cluster roll
treats results below 2 as 2. Aimed preparation still occurs against an immobile target.

Streak SRM-2/4/6 and Clan Streak LRM-5/10/15/20 launchers require a successful
lock before launching. A failed lock still starts the normal recycle time, but
uses no ammunition or heat.
Successful locks hit with every missile; glancing rules do not reduce the salvo.
Streak LRMs have no minimum range, cannot fire indirectly or hotload,
and group their damage into five-point hits like other LRMs. Their recycle times are 15, 20, 25 and
30 seconds respectively. Angel ECM negates the lock: a failed roll still launches
nothing, and a successful one resolves on the ordinary cluster table like a
standard launcher (Tactical Operations: AUE p. 91).

Thunderbolt missiles strike as a single hit; inside the launcher's five-hex minimum
range the unarmed warhead does half damage, rounded down (AUE p. 159). Extended LRMs
inside ten hexes roll on the cluster column for half their rack, rounded up (AUE
p. 139).


`firecluster <selection>` (also `cluster`) toggles artillery between normal and cluster rounds.
`firehoming <selection>` toggles an Arrow IV launcher between normal rounds and homing
missiles, which need a matching ammunition bin.
It requires an intact, recycled launcher and preserves an existing smoke or mine
selection by rejecting the change. Matching ammunition is required to fire.

`lbx <selection>` toggles LB-X autocannons between slug and cluster ammunition.
Use a weapon number, comma-separated numbers or a range, as with `flamerheat`.
The assigned conscious pilot must have a running unit and an intact, recycled
weapon. `weapons` marks cluster mode with `[LBX]`. Cluster fire receives a −1
accuracy modifier, or −3 against a target in the air: an airborne VTOL
(hovering included), a jumping Mech or a dropping unit (flak, Total Warfare
p. 114). A landed VTOL is an ordinary target. Each pellet resolves as one
damage point. Slugs deal the
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

A heavy-duty gyro absorbs its first critical hit with only a +1 piloting
penalty and no fall check. The second hit counts as an ordinary gyro hit (+3
and a piloting check); the third destroys gyro support. The first hit also
preserves an active jump. Heavy-duty gyros weigh twice as much as standard gyros.

`artemis <selection>` toggles compatible missile ammunition on a recycled
launcher with a working linked Artemis IV controller. Selection accepts weapon
numbers, comma-separated numbers and ranges. Artemis rounds exist for standard
LRM (including Enhanced LRM), SRM, MML and torpedo launchers only (Total Warfare
p. 131); Streak, MRM, rocket, ATM, Thunderbolt, extended LRM and Narc
launchers cannot use them, and an ATM's built-in guidance never stacks with
Artemis. One-shot launchers cannot change modes. Compatible rounds use their own
bins; ordinary ammunition
is not substituted when those bins are empty. Artemis adds two to the missile
hit-table roll, combined with the glancing-hit adjustment and capped at twelve.
Spotter-directed indirect fire gets no Artemis or Narc bonus (Total Warfare
pp. 130, 139). On Mechs, vehicles and VTOLs alike, a controller sits in its
launcher’s own section and serves only launchers there (TechManual p. 206).

An MRM "Apollo" fire control system linked to an MRM launcher in its own
section cancels the MRM's +1 to hit and takes one from its Cluster Hits roll
(never below 2), on Mechs, vehicles and VTOLs alike; it needs no special
ammunition and ECM does not affect it. Once destroyed, the launcher fires as a
plain MRM (Tactical Operations: Advanced Units & Equipment p. 143).

`unjam <selection>` begins a 60-second attempt to clear a jammed ammunition feed.
You must be walking or slower, on the ground, with no weapons recycling. Only
one attempt can run at a time. Firing, jumping and running are blocked during
recovery. Every jammed weapon is cleared the way a rotary autocannon is (Total
Warfare p. 140, Tactical Operations p. 73): a successful Gunnery Skill Roll at +3
clears the jam and discards one round. Failure leaves the jam for another
attempt. Empty supply clears it without a check. Shutdown or unconsciousness at
completion ends the attempt without clearing the jam. A feed that broke down (an
Ultra double shot or a caseless round on a to-hit roll of 2) is useless for the
rest of the game: `unjam` refuses it, and only repairing the mount restores it.

Every feed failure happens after the shot: the burst or round is still fired, its
ammunition and heat are spent and its damage is resolved as rolled, and only
then is the weapon disabled (Total Warfare p. 114).

`hotload <selection>` toggles hotloading on supported LRM and extended LRM
launchers. It removes the minimum-range penalty, or halves
it when the game option requests that behavior. Cluster hits use the lowest two
of three dice. Hotloaded launchers never jam (Tactical Operations p. 101). A
hotloaded launcher may explode when critically hit while ordinary ammunition
remains. One-shot launchers cannot change modes.

`ultra <selection>` toggles double-shot firing on supported Ultra autocannons.
Double shots spend two rounds and generate twice the normal heat, with one or
two shells hitting separate locations. With only one round available, the
weapon returns to normal firing. A to-hit roll of 2 in double-shot mode fires
both shots and then leaves the weapon useless for the rest of the game: the
mount stays intact but its feed is jammed beyond field repair (Total Warfare
p. 114).

`rapidfire <selection>` toggles two-round firing on conventional and light
IS autocannons. It spends twice the ammunition and heat, with one or two shells
hitting independently. The weapon returns to normal mode if only one round
remains. A to-hit roll of 3 or 4 jams the feed after the burst; `unjam` clears
it. A roll of 2 fires the burst and then the rounds explode in the barrel,
destroying the weapon and dealing one shot's damage to the internal structure of
its mounting section (Tactical Operations p. 98).

`rac <selection> [1-6]` sets a rotary autocannon's burst length. Omitting
the rate selects one round. Repeating a setting leaves it enabled. Bursts spend
their full ammunition and heat even on a miss; each hitting shell lands
independently. Short ammunition supply resets the weapon to single-shot mode.
Longer bursts jam more easily: a to-hit roll of 2 jams a two- or three-round
burst, 2-3 jams four or five rounds, and 2-4 jams six. The burst that jams is
still fired. A critical hit on a jammed rotary autocannon makes it explode for
one shell's damage. Use `unjam` to recover with a Gunnery Skill Roll at +3.

`gattling <selection>` toggles gatling fire on supported machine guns. Each
attempt rolls damage and heat together, and spends three rounds per damage
point. Low supply limits the result; even one or two remaining rounds can fire
for one damage and one heat. Glancing hits reduce damage without reducing heat
or ammunition spent. The command retains the game's `gattling` spelling.

`precision <selection>` toggles Precision ammunition on conventional and light
IS autocannons. It uses separate half-capacity bins and reduces the target's
movement modifier by two, to a minimum of zero; the −4 for an immobile target
and the prone adjustments are untouched. It can be combined with rapid
fire; weapon inspection shows both selections. It does not fall back to normal
ammunition when Precision bins are empty.

`flechette <selection>` selects Flechette rounds on conventional and light IS
autocannons. Against armored BattleMechs, each shell deals half damage, rounded
down. Flechette rounds use separate ordinary-capacity bins and can be combined
with rapid fire. Selecting the mode again returns to normal ammunition.

`armorpiercing <selection>` toggles AP rounds for conventional and light IS
autocannons. AP bins hold half the ordinary rounds. Shots take +1 to their target
number; every hit that damages armor rolls once for a critical hit, with a
penalty of -1 to -4 that grows as the autocannon gets smaller (Total Warfare
p. 140), and a hit that reaches the internal structure makes only the ordinary
critical roll. Hardened armor negates the roll and takes AP rounds point for
point; laser-reflective armor adds 2 to it. AP can be combined with rapid fire.

`caseless <selection>` toggles caseless ammunition for conventional and light IS
autocannons. Ordinary bins hold twice as many rounds. On a to-hit roll of 2 the
feed breaks down (Tactical Operations: AUE p. 165): the round is spent and
automatically misses, one roll on the Determining Critical Hits Table is applied
to the autocannon's own slots, and the weapon is useless for the rest of the game.
`unjam` cannot clear it; no internal damage or pilot injury results. Vehicles
treat the breakdown as a Weapon Destroyed critical hit.

### Searchlights

Units fitted with `Searchlight` start in automatic mode: the lamp switches on when the battlefield is at night (full moon, moonless or pitch black) and off at dawn, dusk or in daylight, where it gives no benefit. Automatic lamps react when an administrator changes the map's light, when the unit moves to another battlefield, and when the unit finishes starting up. Every switch takes five seconds.

- `slite auto` returns to automatic switching.
- `slite on` and `slite off` hold the lamp on or off regardless of light.
- `slite` alone toggles a running lamp and leaves automatic mode. Repeating it preserves the pending countdown.

The mode can be chosen while shut down and applies once systems are online. `status` shows `SLITE(On/Auto)` or `SLITE(Off/Auto)` for automatic lamps. Lua uses `btech.unit.slite(unit, pilot[, mode])` with a constant from `btech.unit.searchlight_modes` (`AUTO`, `ON`, `OFF`); omitting the mode toggles. `btech.unit.state(unit).searchlight` reports hardware, switch state and mode. Modes, switches and damage survive restart.

An intact active lamp illuminates its carrier and targets within 30 hexes in its forward torso arc, subject to terrain, woods and water obstruction. Illumination follows current positions and facing and feeds optical detection and aiming. Front torso hits can destroy the lamp and cancel its switch.

Clan rotary autocannons (`CL.RotaryAC/2`, `/5`, `/10`, `/20`) support the same `rac` burst controls and `unjam` recovery as Inner Sphere rotary mounts. Burst size, remaining ammunition and the attack roll determine ammunition expenditure, heat and jams; each shell that hits applies its weapon's full damage.

Use `unitprefs SLWarn ON` to receive external searchlight entry/exit warnings, `OFF` to silence them, or `unitprefs SLWarn` to toggle. The preference defaults to off and persists with the unit. Lua provides `btech.unit.searchlight_warning(unit, pilot, enabled)`. Multiple beams produce a single illuminated state; warnings resume after restart without repeating an already observed transition.

`unitprefs ArmorWarn` and `unitprefs AmmoWarn` toggle combat warnings; append `ON` or `OFF` to choose explicitly. Both default to on and persist with the unit. Armor warnings report transitions to low, critical or breached protection separately for front and rear armor. Ammunition warnings use installed-bin weighting and the firing mode's warning window, including shots that miss. Lua exposes `btech.unit.armor_warning(unit, pilot, enabled)` and `btech.unit.ammunition_warning(unit, pilot, enabled)`.

`unitprefs FFSafety ON` blocks weapon fire at units on your team. Use `OFF` to disable it or omit the setting to toggle; the default is off. A battlefield with the no-friendly-fire flag (256) independently blocks these attacks. Lua provides `btech.unit.friendly_fire_safety(unit, pilot, enabled)`.

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


### Physical attacks and the turn

Punches, kicks, trips, clubs and physical weapons reach a target in your own hex or
any of the six adjacent hexes, counted in hexes as weapon range is (Total Warfare
p. 144). Height does not count against that reach; instead the level rules decide:
a standing Mech one level up can be punched, clubbed or struck with a weapon but not
kicked, one a level down can be kicked, clubbed or struck with a weapon but not
punched, and nothing reaches a target two or more levels away (Total Warfare p. 150).
A ground vehicle, or a landed VTOL, can be kicked, clubbed or struck with a weapon on
your own level and punched, clubbed or struck one level up, and takes the blow on a
random side. A flying VTOL is reached by the difference between its elevation and your
hex's level: level with you anything but a punch, one or two levels up anything but a
kick, three levels up only a club or physical weapon, and nothing else. Any physical
blow that lands destroys a VTOL's rotor (Total Warfare pp. 147, 150 and 198).

`punch both #first #second` punches two targets, the left arm at the first and the
right at the second, each rolled on its own (Total Warfare p. 145). A prone Mech can punch only a ground
vehicle in its own hex, propping itself up on the other arm, and both arms are then
tied up (Total Warfare p. 151).

`push [#unit]` shoves the standing Mech in the hex directly ahead of your feet
with both arms, at Piloting -1 and +2 for each damaged shoulder. No arm may have
fired this turn, and the target must be on your level and not charging. A hit does
no damage: the target is driven one hex onward and you step into the hex it left,
and it must make a piloting roll or fall. If it cannot be driven there (the map
edge, terrain it cannot enter, or more than two levels up) neither of you moves,
but it still rolls. Pushed off a drop of more than two levels it falls; a Mech
already in the hex beyond is shoved out of its far side and rolls too, or is
struck by a Mech falling two or more levels onto it (Total Warfare pp. 147,
151-153). A unit can be the target of only one charge, death from above or push
in a turn (Total Warfare pp. 148-149).

A four-legged Mech can kick into its rear arc at +1 with the rear leg on the chosen
side; that leg's actuators count (Total Warfare p. 147). Only two-legged Mechs trip,
and only two-legged Mechs can be tripped; a trip takes Piloting -1 (Tactical
Operations: Advanced Rules p. 89). A club must be picked up with `grabclub` before
you can swing it (Total Warfare p. 146). A charge or death from above that meets its
target number exactly is a glancing blow and deals the target half damage, as other
physical attacks and rams do, with two off every critical roll it causes on a 'Mech or a
vehicle (Tactical Operations: Advanced Rules p. 78).

A Mech makes one physical attack in a thirty-second turn (Total Warfare p. 144).
Punching with both arms, or striking with both claws, in one command is a single
attack; a second physical weapon is not. After a punch, kick, trip, club or weapon
swing no other physical attack can start for thirty seconds, and the weapons in the
attacking limbs cannot fire until then: the punching or swinging arm, both arms for
a club, the kicking leg, or both rear legs when a four-legged Mech kicks behind
itself. A limb that fired a weapon in the last thirty seconds cannot punch, kick,
club or swing, even after that weapon has finished recycling; weapons elsewhere on
the Mech do not matter (Total Warfare pp. 145-147). A charge or death from above
attack needs thirty seconds without any weapon fire, and afterwards no weapon can
fire and no other physical attack can start for thirty seconds (Total Warfare
pp. 148-149). Picking up a club works the same way (Total Warfare p. 146). `status`
shows how long each limb stays tied up.

Use `charge` to select your current target for a ground charge, or `charge #unit`
to select a visible unit. `charge -` cancels. Selection does not start movement:
set your heading and speed to approach the target. A charge is attempted when
movement brings you within collision range. Status shows your charge selection.
Travel toward the target accumulates the charge distance that sets its damage, and the
selection expires after 61 movement updates. Retargeting keeps accumulated time
and distance; cancelling clears them. Jumping ages the timer without increasing charge distance. An in-range airborne
charge attempt clears the selection without attacking. The charging unit takes
one point of damage per ten tons of its target, as in Total Warfare. You cannot
charge within thirty seconds of launching a jump, at a target more than one level
above or below you, or while your pilot is unconscious or stunned (Total Warfare
pp. 148 and 150). A Mech cannot charge a vehicle.

Ground vehicles charge (ram) with the same `charge` command; VTOLs cannot. A ram
can strike a Mech, standing or prone, or another vehicle: tonnage over ten per hex
travelled to the target, on the Kick Location Table against a standing Mech, and the
target's tonnage over ten to the vehicle. After a hit each vehicle involved rolls on
the Motive System Damage Table and a rammed Mech rolls at +2 to stay standing. A
vehicle cannot ram in a turn it fired, and its weapons are held for thirty seconds
afterwards (Total Warfare p. 148). Its anti-missile systems stay ready to defend.


Use `jump #unit` for a death-from-above attack, or `jump` with no arguments to
use your current target. You jump toward the hex occupied by the target when
you launch. If it moves out of that hex, you land normally. Status shows the
selected DFA target during flight. Your Jumping MP must clear the target: the
level of its hex above yours, plus two over a standing Mech, or a VTOL's elevation
(Total Warfare p. 149). A vehicle takes the landing on the Front column of its hit
location table, and a VTOL loses its rotor (Total Warfare pp. 150 and 198).
After a hit you roll at +4 and the target at +2 to stay standing; each roll stands
alone, so a target still rolls when your landing damage has already knocked you
down (Total Warfare p. 150).

A successful attack damages the target and your legs. A miss is a two-level fall
onto your back: every hit strikes your rear and your pilot makes the fall's
piloting check at +1. Both outcomes tie up every section for thirty seconds, as
described above. An accepted DFA uses this
physical recovery instead of ordinary jump stabilization. `land` can trigger the
attack early if you are already in the target's hex.


### Anti-missile defense

`ams` toggles all installed anti-missile systems on the unit. Defense starts on
for newly built units with AMS installed.
A running unit with defense enabled automatically engages an incoming missile
attack that reaches its base target number with one ready mount whose firing arc
covers the attack direction (Total Warfare pp. 129-130). Missed attacks and
attacks from outside every mount's arc preserve defensive ammunition, heat and
readiness. Defensive weapons cannot be fired manually.

An engagement applies -4 to the attack's cluster roll, never below 2; a locked
Streak launcher counts as a roll of 11 before the -4. Electromagnetic interference
takes two more from either. A single-missile attack
(Narc and iNarc pods, Thunderbolts) instead rolls one die and is destroyed on a
1-3. Each engagement spends one round of ammunition and the system's heat, and
each AMS engages at most one attack a turn: thirty seconds pass before that mount
can engage again. Inner Sphere and Clan systems follow the same rules. Each AMS
works on its own: one with an empty bin, or one destroyed by a critical hit,
leaves the attack to the next ready mount covering it. Lua can toggle with
`btech.unit.ams(unit, pilot)` or set an explicit state with a third boolean
argument.

Laser AMS uses the same switch but needs no ammunition. IS laser AMS generates
7 heat per engagement and Clan laser AMS 5.

### Machine gun arrays

A machine gun array links the two to four machine guns of one size class in its
location that its design lists, on a Mech, vehicle or VTOL (TechManual p. 228).
Firing any linked gun fires every linked gun that is ready and not set to
rapid fire, as one attack: one to-hit roll, then a roll on the Cluster Hits
Table column for the number of guns, every hit striking one location. Each gun
spends its own round and recycles. A shot at a hex fires the array the same way,
its hits striking the hex as one blow. When the bins cannot feed every gun, the
gun you fired draws first and only the guns that get a round fire; the Cluster
Hits column follows the guns that fired, and you are told how many did. A gun
in rapid fire, or one left alone after the others are destroyed or recycling,
fires by itself, and so does every gun once the array is destroyed (Total
Warfare p. 137).

### Torso-mounted cockpits

A torso-mounted cockpit sits in the center torso. Head hits do not injure you,
and losing the head neither destroys the Mech nor kills you: later hits there
are rolled again. Losing the center torso, which holds the cockpit, kills you. You add one to every piloting roll. The extra sensor package
means a second sensor hit only raises the weapon penalty from +2 to +4; a third
stops weapon attacks. Once both head sensors or three sensors are gone you are
blind, adding 4 to piloting rolls and physical attacks. With life support
damaged you take a point of damage each turn at 1-14 heat and two at 15 or more
(Tactical Operations: Advanced Units & Equipment p. 113).

### Talons and spikes

Clan talons fill two slots in every leg. A kick from a leg whose talons and foot
actuator both survive deals half again its damage, rounded up, after triple
strength myomer; death from above deals half again while any leg's talons work
(Tactical Operations: Advanced Units & Equipment p. 103).

An Inner Sphere spike cluster takes four points off every physical blow that
lands on its location, never below one. A bare arm or leg that strikes it takes
two points itself: the punching arm, the kicking leg, a point to each leg after
death from above, or the center torso after a charge. Clubs and physical weapons
strike without touching the spikes. Each working cluster in a torso location
adds two points to your own charges; after a charge, each damaged location of
yours that holds a cluster loses it on 9 or more on 2d6.

### Anti-infantry pods

A-Pods and B-Pods are one-shot defenses against infantry, which this game does
not field, so neither has a target and firing one does nothing. A B-Pod weighs
a ton in one slot anywhere on the Mech (TechManual p. 204); a critical hit on
it, on a Mech or a vehicle, releases 2 points of damage in its location (Total
Warfare p. 130).

### Narc beacons

IS and Clan Narc launchers fire homing pods. A pod attaches to a surviving section
without damaging armor or structure. AMS engages it as a single missile: one die,
destroyed on a 1-3. Destroying the marked
section removes its beacon; otherwise the mark persists across shutdowns and restarts.

`narc <weapon>` toggles Narc-compatible ammunition on a standard LRM, SRM or MML
launcher (Total Warfare pp. 141-142); Streak, MRM, rocket, ATM, Thunderbolt and extended
LRM launchers fire only ordinary rounds. Compatible missiles gain a
cluster bonus against a marked target and require a matching bin.
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

`commseccm` switches communications equipment's ECCM field on or off. Three to
six tons of working communications equipment generate half a Guardian suite's
ECCM and seven or more tons a whole suite's, over the same area; it never
generates ECM, and it stops when its equipment, the unit's sensors or a Mech's
cockpit is damaged (TechManual p. 212; Tactical Operations: Advanced Rules
p. 98).

ECM protects friendly units and disturbs enemies within six hexes, twelve under
electromagnetic interference, however high either unit is. Enemy ECCM
cancels protection, and friendly ECCM cancels disturbance; equal strength cancels.
Angel suites count twice. Team identifiers determine which units are friendly.

Protection on a target or disturbance on a shooter suppresses Narc and Artemis
guidance bonuses. Angel interference also negates Streak locks: a failed roll
still launches nothing, and successful attacks use ordinary missile cluster rolls.

Status includes suite selections and current field effects. Lua provides
`btech.unit.ecm(unit, pilot)`, `eccm`, `angelecm`, `angeleccm` and `commseccm`,
each returning `"off"`, `"ecm"`, or `"eccm"`. Unit inspection includes the saved `electronics`
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
intact, free of physical recovery, and must not have fired a weapon in the last
thirty seconds (Total Warfare pp. 141, 220). Actuator damage makes the attempt
harder. A failed attempt damages the selected location, and either outcome leaves
the arm recovering for 30 seconds. Conventional Narc pods cannot be swatted off.

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
conscious pilot must be in a running, placed unit with a working Guardian or
Angel ECM suite. Stealth armor cannot be combined with a null signature system.
Repeated requests leave the current transition unchanged.

Active armor produces ten additional heat and disrupts your own electronic
guidance. Enemy radar and probe sweeps take +4 to +7 against you (Tactical
Operations ECM/Stealth table), so enemies mostly rely on sight, infrared,
magscan and seismic sensors, or a Bloodhound probe. Enemies firing at you take
range penalties of 3 at medium range and 6 at long range. Short and minimum
range are unchanged. While the armor is engaged its ECM suite powers it and makes
no ECM or ECCM field, and your own active probe is jammed. An Angel suite set to
ECCM (`angeleccm`) keeps its ECCM field, but then the armor loses every effect
but its ten heat (Tactical Operations: Advanced Rules p. 98).
Shutdown or ECM damage disables active armor. A pending switch only completes
if the unit is running and its ECM works when the delay expires.

Lua provides `btech.unit.stealth(unit, pilot)`. Unit inspection exposes `stealth`,
including its active selection and any pending switch.

Firing at an active stealth target requires a lock on that unit. Missing or
different-target locks reject the shot before ammunition, heat, recovery or
attack dice change. Normal visibility and firing checks still apply.

### Null signature system

`nss` toggles an installed null signature system after a 30-second delay. It
requires the assigned conscious pilot, a running placed unit, and intact devices.
A repeated request does not restart a pending switch.

Active NSS adds ten heat and raises enemy range penalties to 3 at medium range
and 6 at long range. Enemy radar and probe sweeps take the
Tactical Operations ECM/Stealth modifier against you; infrared, magscan and
seismic sweeps do not. It does not create ECM interference or require enemies
to hold a firing lock. If stealth armor is also active, the
range penalty applies once and the two systems' heat adds together.

Shutdown or device loss disables NSS. Pending switches only take effect if power
and devices are available at expiry. Lua uses `btech.unit.nss(unit, pilot)`; unit
inspection includes `null_signature` and its pending destination/countdown.

### Active probes

A working Beagle, Light, Watchdog or Bloodhound Active Probe lets you ignore one
point of woods when aiming at a target within four, three, three or eight hexes
(Clan probes five) that it has a clear line to. It also sweeps with your sensor
suite, ahead of the built-in sensors, using its longer brackets (Beagle 12/24/36,
Clan 15/30/45, Light and Watchdog 9/18/27, Bloodhound 16/32/48); nothing blocks
a probe sweep but the surface of water, and a target it finds shows as a probe
contact. Guardian and Angel ECM, stealth armor and null signature systems add to
the sweep roll against them. Hostile ECM jams a probe: inside an enemy ECM bubble,
or with your own stealth armor engaged, it no longer sees into woods, and no probe
sees into woods inside the enemy's own ECM bubble. A Bloodhound ignores Guardian,
Clan and Watchdog ECM but not an Angel suite, and cannot be used at all while your
stealth armor is engaged. A jammed probe still sweeps, taking the ECM penalty. A
damaged probe stops working; `sensor` shows its condition. Electromagnetic
interference renders every probe useless: it neither sweeps nor sees through
woods while the condition lasts.

### Radar

Your unit's radar sweeps with the rest of the suite: 8/16/24 hexes for Mechs,
6/12/18 for vehicles. Hills and buildings block it, heavy woods shorten it and
ECM and stealth systems add to the sweep roll. It has no special treatment of
airborne targets.

Use `tag ID` or `tag #unit` to make a TAG attack on an enemy within fifteen
hexes (nine with only a Clan light TAG) with working TAG hardware and an
unobstructed line of sight. Designating is a weapon attack (Total Warfare
p. 142): it rolls 2d6 against your ordinary to-hit number with TAG's own range
brackets of 5/9/15 hexes, or light TAG's 3/6/9 (TechManual p. 343), and no
minimum range. Your
movement, heat and sensor damage, the target's movement and cover, the light and
the secondary-target penalty all count, as does a Haywire pod on your unit; TAG
has no ammunition or targeting-computer terms. A miss designates nothing and the
TAG recycles for thirty seconds like a fired weapon. Mechs and vehicles share
this control, including TAG built into C3 master computers. A hit takes thirty
seconds to settle; you cannot change it during that timer. Use `tag -` after
settling to stop. Breaking a connection starts thirty seconds of recycling.
Moving out of range, blocked sight, lost sensor contact, shutdown or lost
equipment breaks the connection. Another tagger can take over the same target.
TAG state appears in unit inspection. Lua uses `btech.unit.tag(unit, pilot,
target)` and nil to stop.

Use `sguided <weapon>` to select semi-guided rounds on a standard LRM launcher
(IS, Clan or Enhanced) or an MML loaded with long-range rounds (Total Warfare
p. 142); repeat it for normal rounds. Against a target a friendly unit's TAG
designates, your own included, semi-guided missiles ignore the target's
movement modifier; the −4 for an immobile target and the prone adjustments
remain. Fired indirectly at such a target, they also ignore the +1 indirect
modifier, the spotter's movement and the cover the spotter sees. Losing the TAG
removes the benefit. The launcher needs matching semi-guided ammunition; normal
rounds are not substituted. Lua uses `btech.unit.sguided(unit, pilot, weapon)`.

Use `spot #your-unit` to declare yourself a spotter; `spot -` ends that role. A
spotter keeps firing its own weapons (Total Warfare p. 111). While another unit
fires through you, your own weapon attacks take +1, and an indirect shot you
spot takes a further +1 if you launched a weapon within the last thirty seconds.
A unit carrying an iNarc Haywire pod cannot spot, and neither can one that is
charging or making a death from above attack, or did so within the last thirty
seconds. A friendly unit selects you with `spot #your-unit`; it needs no sight
of you, only a declared, running, conscious spotter on its battlefield.
Selecting another spotter restricts firing to indirect-capable missiles and
artillery until `spot -` clears the link. Lua uses `btech.unit.spot(unit, pilot,
observer)`.

For indirect missile fire, select a friendly spotter, clear your own target with
`lock -`, and use `fire <weapon number>`. Your observer must have an acquired unit
target. Indirect fire adds +1, your observer's movement modifier (walked +1,
ran +2, jumped +3, prone +2) and +1 if it also attacked this turn; terrain and
cover come from the observer's view of the target, while range, including
minimum range, is measured from you. An attacker that can see its target
itself may not fire indirectly: with a spotter selected and no target lock,
the observer still supplies the target, but the shot resolves as direct fire
from your own sight of it. Keeping your own unit target lock uses direct missile
fire. Without that lock, your observer supplies the target even when you give
`fire` an explicit target ID.

For artillery, your observer must select a visible hex. The observer must remain
running, friendly, conscious and on your battlefield. Artillery fires at the
observer's hex. Clear any unit target lock before firing artillery. Retargeting the
observer resets accumulated correction. Artillery reach follows Tactical Operations
(p. 149): Arrow IV 8 mapsheets (Clan 9), Thumper 21, Sniper 18 and Long Tom 30, at
seventeen hexes per mapsheet; a hex beyond it cannot be fired at.

Artillery you fire at a hex you can see within seventeen hexes is direct fire
(Tactical Operations p. 153): your artillery gunnery +4, plus your movement, heat,
sensor and actuator damage and the woods or smoke you fire through, with no range
or target modifiers. Direct fire cannot be made at a hex within six hexes. A
direct-fire miss scatters 1D6 hexes. Every other artillery shot is indirect
(pp. 148-150): gunnery +7, plus your observer's spotting modifier and +1 if it also
attacked within the last thirty seconds, less one for each corrected miss, plus your
movement when the hex lies within seventeen hexes.
An indirect miss scatters one hex for each point it missed by. A miss scatters in
a direction rolled on one die, clockwise from north; a round that scatters off the
map lands harmlessly. Once one of your rounds strikes its target hex, every later
shot you fire at that hex hits it automatically, until you move, turn or jump.

Arrow IV homing missiles (Tactical Operations: AUE p. 166) are fired at a hex like any
artillery round. When one arrives it attacks a unit that a TAG on your side designates
within eight hexes of that hex: the one you have locked if it is designated, otherwise the
designated unit nearest the hex. On 4 or more on 2D6 it strikes that unit for 20 points in
5-point groups from your direction, and 5 points strike every other unit in its hex; on 3
or less it strikes the hex for 5 points to every unit there. With no designated unit in
reach when it arrives, it falls on the hex it was fired at as an unguided round, 5 points to
every unit there, scattering on a miss like any other round. In the book the firer chooses
between these two attacks when firing; in realtime the missile chooses on arrival, since
nobody can know at launch whether a designation will hold. A homing missile never makes its
hex an automatic hit.

An artillery round that strikes a building hex damages every unit inside a light,
medium or heavy building in full, whatever floor it stands on, and every unit on its
roof (Tactical Operations p. 152). In a hardened building the round bursts on the
roof: the roof and the level below take its full damage, the next level half and
the floors below nothing. The building then loses three times the round's damage,
plus twice it for the level below the roof and once for the level below that; a
building, wall or bridge in the ring around the struck hex loses twice the ring's
damage, plus once for the level below its roof (Total Warfare p. 174).

Artillery cannons are ballistic weapons that deal artillery damage (Tactical
Operations: Advanced Units & Equipment p. 97): one that sees its hex aims with its
own range brackets and minimum range and no hex bonus, and a miss scatters half the
margin it missed by, rounded up, never farther than the target lay. A cannon's
round deals its damage in the target hex and ten less in each ring around it.

Use `lock x y` to select coordinates, or append H (hex), B (building), I (ignite),
or C (clear). Coordinate locks take effect at once, even outside visibility.
`lock -` clears the selection. With a plain coordinate lock, `fire <weapon>`
attacks its current unit occupant under ordinary firing rules. The lock stays at
the coordinates when that unit moves. A shot at the hex itself (a plain lock on an
empty hex, or an H, B, I or C lock) treats it as an immobile target at -4 (Total
Warfare p. 110) and pays for the woods, smoke and crops between you and it, or
between your spotter and it for indirect fire (pp. 107, 111). The hex's own woods and
smoke add nothing, since a hex does not stand in itself. Artillery always fires
at the selected hex and arrives after a flight delay, applying its ammunition
payload to the area.
Without a spotter, artillery may still fire at a hex you cannot see (Tactical
Operations p. 148), with no extra modifier, but that fire cannot be adjusted: a
miss earns trajectory correction only while a friendly unit can see the target hex.
Underground, artillery fired at your locked hex needs an observer or your own sight
of it.


Crossing into a mined hex, landing or falling can trigger its mines. Conventional
and inferno fields detonate on a 2d6 roll that gets easier as the field grows
denser (9+, 8+ or 7+); a falling Mech or a hovercraft sets one off only on a 12,
and each blast can thin the field by five points. Inferno mines heat a Mech like
inferno missiles. Vibra mines detonate under a Mech heavy enough for their
setting, from up to a few hexes away for much heavier Mechs, and damage only
that hex. Active mines go off under a landing jumper. Small bomblets spotted on
the ground are command mines; spotting them does not detonate them.


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
range with clear line of sight. Scanner range is 25 hexes; one sensor critical
halves it and a second disables it.

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
locks inspect visible occupants. Observer units may scan selected coordinates
beyond hardware range, but still require visibility and working scanners.


## Brief unit reports

Use `report AB` (or a unit dbref) for a brief, silent view of a visible contact.
It shows identity, position, motion and heat without armor or weapon details.
Use `report x y` for a visible occupant at a coordinate, or `report` for your
current target. Direct unit reports can reach beyond detailed scan range, but
still require a visible acquired contact and working scanners. Coordinate reports
must remain within scanner range. Reports do not alert the target.


## Long-range maps

`lrs` and `lrsmap` invoke the same display. The first letter selects the mode,
so `lrs Terrain` and `lrs Combined` select T and C. `lrs Units` selects M, the
units mode; other words beginning with U select underlying terrain.

Use `lrs T` for terrain, `lrs E` for elevation/depth, or `lrs M` for
terrain with visible units. Your unit appears as `*`, friendly bipeds as `b`, and
hostile bipeds as `B`. Dark maps mark obscured hexes with `?`.

Add a contact label/dbref to center on it, or a bearing and signed distance to
project the center: `lrs T 180 20`. Negative distance looks behind that bearing.
Display windows stay inside the map. You must be the conscious assigned pilot
of a running unit with working scanners. These displays do not acquire contacts
or alert other units.
A long-range display can center on a contact or point up to 50 hexes away; one
sensor critical halves that reach and a second disables the display.

Use `lrs C` for terrain-colored elevation, including explicit zeroes. With ANSI
enabled, terrain and unit modes use map colors: your marker is bold, friends are
yellow, enemies and fire are red, woods green and water blue. Ordinary `lrs E`
remains uncolored and leaves zero elevation blank.

Use `lrs U` to see the terrain beneath fire and smoke. Use `lrs L` to filter terrain
by current visibility, `lrs H` for filtered elevation, or `lrs S` for visible units
over filtered terrain. These modes work
on ordinary maps as well as dark maps. Question marks mean the requested terrain
information is obscured.

Active fires and burning stationary units light nearby terrain. Forward searchlights
can reveal terrain farther away at night, but obstructions block their beams.
Visibility-filtered maps update as those light sources move or expire.


`tactical [C|T|B|M|L] [target | bearing range]` draws nearby terrain, elevations and
known contacts in a hex grid. Your unit is `**`; friendly contact IDs are lowercase
and enemy IDs uppercase. Fire (`&`) and smoke (`:`) fill the top of a hex, and the
terrain beneath them stays in the bottom. Use `L` to show only visible terrain. Dark maps always hide unseen terrain. Omit the center to
use your unit, supply a known contact ID, or give a bearing and signed distance.
The display needs a running unit and working tactical sensors. Use `navigate` for a local hex-shaped view.
A tactical display can center on a contact or point up to 25 hexes away; one
sensor critical halves that reach and a second disables the display.


`tactical C` highlights mech cliffs (three or more elevation levels);
`tactical T` highlights tank cliffs (two or more). Both show your own marker
and elevations, leaving other contacts out of the display. Water and ice depths
count below ground level. Colored displays highlight cliff edges in red; plain
displays use `|`, `!` and `,`. These overlays are unavailable on dark maps.


`tactical B` marks potential landing hexes with O (suitable) or X (unsuitable).
Suitable terrain is grass, road or sand clear of fire and smoke, with six
equal-height neighboring hexes, outside landing restrictions that apply to your team. It shows your own marker but omits other contacts,
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
terrain, speed and heading, plus an effect line when fire or smoke covers your hex.
Below the heading, one line per layer of your hex spells out what the terrain symbol can
hide: its ground or water depth and flow, any woods, jungle or fields, a road or rail line,
a building, wall or bridge with its construction class and remaining CF, and any ice, snow
or mud. Omit arguments to center on yourself. This view remains
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


`contacts` also lists targets that only running units on your side can see
(Tactical Operations: only targets spotted by a friendly unit may be attacked).
Such rows carry no detection letter, since your own sensors do not hold them,
and identify the target when a teammate does. You may fire at them when you
have your own line of fire. When your unit is in an active C3 or C3i network,
every row gains a `c:` column after `r:` with the shared targeting range: the
closest usable sighting in the network, which is what your weapons aim with.
`contacts #unit` reads a side-spotted target the same way.


The five characters after `S:` summarize contact condition. They show, in order:
carried club (C); destroyed (D), lamp (L) or illuminated (l); jumping (J), prone (F)
or standing transition (f); shutdown (S), starting (s), excess heat (+) or, for a
stationary unit, burning jelly (I); and homing beacon (n friendly/N hostile), ECCM (P), ECM (E), protection (p)
or interference (e). A higher-priority condition in a column hides the others.


Contact rows begin with how you currently perceive the contact: `V` sight, `S`
a built-in sensor or `P` a probe sweep; a blank means only a teammate perceives
it.


After that letter, `*` means the contact is in your forward torso arc,
`r` right, `l` left, and `v` rear. Buildings use the same symbols. Torso twists
change this indicator. Individual weapons can have different arcs, so the symbol
does not guarantee that every weapon can fire.


Automatic notices use long sentences in A0/A2 and short Seen:/Lost: messages in
A1/A3/A4/A5. Notices include the contact label and arc. A0–A3 color enemy arrivals
red and losses yellow when your terminal supports color; A4/A5 use no color.
A2/A3/A5 show enemies only, and A6 disables routine notices.


A contact found by sensors rather than sight appears as "something". Its
condition columns stay blank, and it is not classified as friendly until
identified.


Routine notices normally skip shutdown targets. Use `unitprefs AutoconShutdown ON`
to include them, or OFF to restore the default. This changes notices, not which
contacts appear in your list, and never suppresses weapon-lock loss warnings.


Compact unit rows use battlefield labels such as [ab], followed by B for a biped.
The columns show x/y/z position, r weapon range in hexes, b bearing, s speed,
h heading, and S status.
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
travel. Only quads with all four legs intact can move laterally. A lateral shift costs
one movement point more in every hex, and a shift into a rear side hex is backward
movement, held to walking speed and taking the backward level-change check (Total
Warfare p. 50). A change takes six seconds; requesting
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
ground distance appears alongside it when different. Weapons do not use either:
they count whole hexes, as `contacts` shows. Dark maps hide terrain
height differences from coordinate measurements. Unit targets must be visible.

Use `vector` to combine range and bearing to your selected target. You can also
use `vector x y`, `vector x y z`, `vector x0 y0 x1 y1`, or
`vector x0 y0 z0 x1 y1 z1`. Explicit heights and default targets include a vertical
angle marked + for above or - for below. Vector retains terrain elevations even
on dark maps.

Use `heading` or `speed` without a value to read your current motion.
`speed cruise` equals `speed walk`; `speed flank` equals `speed run`. Walking or
cruising speed is your template's walking or cruising MP after damage, and running or
flanking speed is that figure x 1.5, rounded up; you may not back up faster than walking
or cruising speed. Numeric speed requests are limited to your current forward/reverse
throttle range.
Movement restrictions such as water and weapon unjamming still apply.

Use `dump all`, `dump <weapon number>`, `dump <section>`, or
`dump <section> <critical slot>` to eject ammunition. Weapon numbers start at zero;
critical slots start at one. For example, `dump RT 1` selects the first right-torso
slot. `dump stop` cancels. Dumping takes time and prevents running and jumping.

`masc` toggles an installed MASC booster, which lets you run at twice your walking
MP (Total Warfare p. 137). Every thirty-second turn in which it was switched on ends
with a 2d6 activation check: the first such turn fails on a 2, the second on 4 or
less, the third on 6 or less, the fourth on 10 or less, and a fifth consecutive turn
fails automatically. Each turn you leave it off steps the interval back down, never
below the first. A failure puts one critical hit on each surviving leg, located as a
normal critical, and the MASC keeps working. `weapons` and `status` show the
current interval.

`scharge` toggles a supercharger, which follows the same activation checks (Tactical
Operations: Advanced Units and Equipment p. 345). A failure rolls on the Determining
Critical Hits Table, applies the result to the upper-most undamaged center torso
slots, and destroys the supercharger for the rest of the scenario. With both engaged
you run at two and a half times your walking MP, rounded up, and each device makes
its own check with no extra penalty. `weapons` and `status` show the separate
intervals.


Artillery also supports `hotload <selection>`. Hotloaded launchers never jam. An
intact hotloaded launcher can explode if critically hit while ordinary
ammunition remains aboard.
Hotloading leaves the selected artillery payload and its aiming rules unchanged.

Ground vehicles also accept `lbx`, `sguided`, `precision`, `flechette`,
`armorpiercing` and `caseless`, with the same Lua operation names.
These change the ammunition selection on compatible, intact, recycled weapons.


Inferno hits on moving-capable ground vehicles and VTOLs follow Total Warfare: every
missile that strikes rolls once on the vehicle's critical hits table for the location
hit, at −2 (Total Warfare p. 142). Infernos deal no armor damage to a vehicle.
Stationary installations instead accumulate burning-jelly duration. Vehicles have
no heat scale, so blast heat affects only stationary installations.

## Entering a hangar

At a building entrance, use `enterbase [direction]`. The unit must be running and
moving slowly; Mechs must be standing, and VTOLs must be landed with fuel. An
omitted direction uses the first arrival point. The doors take eighteen seconds to
open. Your current route, movement condition and building lock are checked again
when that delay expires. Moving away or losing eligibility cancels entry. Pushing
through the walls is a Total Warfare hazard: you roll against the entrance's
construction class, at +0 for light, +1 medium, +2 heavy and +3 hardened, and a
failure costs you a tenth of the building's remaining integrity in damage from
the front. The building loses a tenth of your tonnage either way.

Entry also follows teleport policies. A damaged building permits forcing its enter
lock only when it is unsafe and below half its maximum integrity.

## Buildings on the battlefield

Building hexes follow Total Warfare. Walking or driving into a building hex
below its roof puts you inside it, at the level you came from (no lower than
its ground floor); arriving at roof height, from a hill or a jump, puts you on
the roof, which is ordinary terrain. Every building hex you enter below the
roof costs a piloting or driving roll at +0 for a light building, +1 medium,
+2 heavy and +5 hardened; a failure costs you a tenth of the hex's construction
factor from the front (the rear when reversing), and a vehicle that fails also
rolls for motive damage. The hex loses a tenth of your tonnage either way, and
collapses when that spends it or when the units sharing your level above the
ground floor outweigh what is left. Inside, each hex costs a light building one
extra movement point, a medium two, a heavy three and a hardened four.

A unit inside a building is seen and shot at by its level; the building's own
hex never blocks the line. A standing Mech one level below the roof has partial
cover from every direction. Every hit grouping fired at a unit inside from
another hex loses a tenth of the hex's construction factor to the building,
which takes the same, and a miss from the next hex strikes the building. A shot
from another level of the same building hex hits a Mech on the Special Hit
Location Table: from above it finds the arms, torsos and head, from below the
legs and torsos (Total Warfare p. 175). Punches and kicks cannot reach a unit
inside from outside its hex. When a hex collapses,
every unit inside takes a tenth of the construction factor for its own level and
each level above it, in five-point groups from the front, and stands on the
rubble; units on the roof fall. Aircraft, VTOLs and naval units cannot enter a
building hex. Buildings with an entrance into a separate interior map are
unchanged; see "Entering a hangar".

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

Vehicles cannot dig in. Total Warfare gives them no such cover, and the Tactical
Operations hull-down rule for vehicles needs the half-level hexes maps do not have.

Mech pilots can use `hulldown` to lower the chassis, `hulldown -` to raise it,
and `hulldown stop` to cancel a pending change. A quad squats; a biped takes a
knee, and must pass a piloting roll to get it off the ground again, otherwise it
stays down with the time spent and may try again. Transition time depends on
current maximum chassis speed. Finish standing or jump stabilization first.
Lowering requests a stop. Speed, heading, standing and jumping controls are
blocked while lowered or changing posture. Leg-mounted weapons, including a
quad's front legs, cannot fire while lowered (Tactical Operations p. 19); the
other weapons fire normally.

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

`heatcutoff <sinks>` shuts off that many of a Mech's heat sinks, most often to
build up heat for triple-strength myomer; `heatcutoff all` shuts off every
working sink and `heatcutoff 0` switches them all back on (Total Warfare p. 159,
Shutting Off Heat Sinks). `heatcutoff` alone reports the current setting. It
needs the conscious assigned pilot; the reactor may be off.

The count is in sinks, so a double heat sink is one sink worth two points of
cooling. A request takes effect at the unit's next 30-second turn boundary,
whether it shuts sinks off or switches them back on, and a newer request
replaces one still waiting. Losing a heat sink to damage lowers the count to
the sinks that still work. Water doubles only the sinks left on, and other
environmental effects still apply. `status` shows the resulting heat
production, heat sinks and dissipation.


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

Use `explode` (or `explode ammo`) from a running unit to start a sixty-second
self-destruct countdown. In-character units require the assigned pilot, except
for wizards; out-of-character units allow other cockpit occupants. The command
requires map placement, completed weapon and physical recovery, and live
destructive ammunition. Gauss ammunition does not qualify.

Engagement releases the pilot assignment. `explode stop` aborts the countdown;
on an in-character unit, a non-wizard must take the cockpit again first.
Shutdown, destruction or losing the last destructive ammunition cancels a
pending sequence. Wizards can append lowercase `override` to use a
three-second delay and bypass the cooldown and ammunition-safety gates; cockpit
authority and the ammunition-presence check still apply.

When the countdown ends, a Mech's destructive ammunition explodes bin by bin
with the ordinary ammunition-explosion rules. Vehicles instead lose their rear
section and receive a crew injury. Fusion reactors never explode.


## Dropping prone

Use `prone` in a running biped or quad with its assigned pilot in the cockpit.
The command stops your travel, centers your torso, restores flipped arms and
cancels hull-down posture. You cannot use it while airborne or trying to stand.

Dropping to the ground is a deliberate 1 MP action (Total Warfare p. 49): it
needs no control roll, causes no damage and adds no heat, whatever speed you were
moving at, forward or backward, and you keep your facing. You may not drop within
thirty seconds of launching a jump. Dropping into water can flood exposed
compartments, and the final ground contact activates stepping mines.

Firing when down adds +2 to your weapon attacks (Total Warfare p. 113); a biped
propped on its only remaining arm takes +3 instead (Tactical Operations p. 83).
A four-legged Mech with all four legs intact and no hip critical fires as
though standing, with no prone modifier. Targets shooting at a prone Mech get
−2 from an adjacent hex and +1 from farther away.


### Swarm missile ammunition

`fireswarm <weapon>` selects Swarm rounds; `fireswarm1 <weapon>` selects Swarm-1.
Repeat the same command to return to normal rounds. These commands accept the
usual weapon selections and work on Mechs, ground vehicles and VTOLs. A matching
ammunition bin is required to fire. Swarm rounds are LRM munitions: only standard
LRM launchers and MMLs loaded with long-range rounds can use these modes.

Missiles that do not strike a target fly on to the nearest unit, friend or foe,
in its hex or an adjacent one, nearest first and at random among equals; no line
of sight is needed, and a miss sends every missile on (Tactical Operations: AUE
p. 183). Swarm can hit friends and return to its launcher. Each later attack keeps
the first attack's modifiers but takes its own target's movement and terrain
modifiers, and its Cluster Hits roll uses the column for the missiles still
flying. Swarm-1 takes +2 against friendly units, unless they stand in hostile ECM
or have lost sensors. No unit is attacked twice, and combat-safe units are
skipped. In hostile ECM, Swarm-1 acts as Swarm, and a Swarm flight attacking a
target in hostile ECM acts as an ordinary LRM flight and flies no farther. An AMS
takes no -4 from a Swarm flight: the missiles that attack its unit land as though
an 8 were rolled on their own column, and the rest fly on.
Empty-coordinate fire does not retarget.

Lua provides `btech.unit.fireswarm(unit, pilot, weapon)` and
`btech.unit.fireswarm1(unit, pilot, weapon)`. A Swarm firing result contains
`salvo.kind = "swarm"` and ordered attacks in `salvo.report.hops`.


### Smoke and mine missile modes

`firesmoke <selection>` and `firemine <selection>` select Smoke or Mine ammunition
on missile launchers. Smoke rounds load in standard SRM, LRM and MML launchers;
Mine (Thunder) rounds load in standard LRM launchers only, and the Thunder-Active,
-Augmented and -Vibrabomb rounds (`fireactive`, `fireaugmented`, `firevibrabomb`)
in Inner Sphere ones. Repeat the selected command to return to normal ammunition.
Matching supplies are required to fire. Both controls work on Mechs and vehicles;
other launchers, one-shot launchers and artillery are rejected.

Smoke and Thunder missiles may only target hexes and damage no unit (Tactical
Operations: AUE pp. 183, 185): fire them at a coordinate or a hex lock, whoever
stands there. A Smoke hit fills the hex with smoke until the End Phase three turns
later: light smoke when the salvo delivers 5 points of damage or less, heavy
otherwise. A Thunder hit lays a minefield as dense as the launcher is large (20
for an LRM-20); Augmented rounds lay half that, rounded up, in the hex and every
hex around it. A Thunder miss scatters as artillery does, 1D6 hexes from direct
fire or the margin of failure from indirect fire, and lays five points less.
Weapon-delivered fields in one hex total at most 20 points, except that a field
placed before play may be thickened to 30.
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
is recycling or out of ammunition, and preserves weapons hold.
It consumes the aiming dice but never fires, spends ammunition, adds heat,
or clears a jam. Destroyed and defensive-only weapons cannot be sighted.

`weaponstatus` lists every installed weapon's condition, including preferred
ammunition sources and destroyed or disabled slots. A conscious passenger may use it while the unit is on a map,
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
and disabled equipment, ammunition quantities and capacities, spent
one-shot launchers, rear mounts and Artemis links. Split slots identify their
linked weapon. You must be the conscious assigned pilot, but the unit may be
shut down or off-map.

`melee [left|right|both] [#unit]` swings the physical weapon installed in each
selected arm. Arms default to both, and each arm uses its own weapon, so a Mech
with a sword in one arm and a claw in the other needs no separate command for
either. Arms without a physical weapon are skipped when both are selected. Claws
may strike with both arms in one action, as punches do; any other weapon swings
once, and that swing is the turn's physical attack, so the second arm does not
swing. Use `punch` for bare fists.

| Weapon | Template name | Damage | To-hit | Hand needed |
|---|---|---|---|---|
| Axe | `Axe` | tons / 5 | -1 | yes |
| Sword | `Sword` | tons / 10 + 1 | -2 | yes |
| Mace | `Mace` | tons / 4, rounded up | +1 | yes |
| Dual saw | `Dual_Saw` | 7 | 0 | no |
| Claw | `Claw` | tons / 7, rounded up | +1 | no |
| Retractable blade | `Retractable_Blade` | tons / 10, rounded up | -2 | yes |
| Lance | `Lance` | tons / 5, rounded up | +1 | no |
| Flail | `Flail` | 9 | 0 | no |
| Wrecking ball | `Wrecking_Ball` | 8 | +1 | no |
| Chain whip | `Chain_Whip` | 3 | -2 | yes |
| Small / medium / large vibroblade | `Small_Vibroblade`, `Medium_Vibroblade`, `Large_Vibroblade` | 7 / 10 / 14 | -2 | yes |

Triple-strength myomer doubles axe, sword, mace, claw, retractable blade and
lance damage only. A missed mace swing forces a piloting check at +2. A roll of
2 with a flail or wrecking ball swings it back into your own front for 5 or 4
damage, followed by a piloting check. A wrecking ball reaches only your forward
arc; every other weapon also reaches its arm's side arc (Total Warfare p. 146).
Claws strike as punches, so a damaged shoulder stops them. Each vibroblade swing
adds 3, 5 or 7 heat.

When a lance hits and armor still covers the struck location, roll 2d6: on 10 or
more, one point of damage goes straight to internal structure, and its critical
hit roll takes a -2 penalty. Once that location's armor is gone, the lance hits
normally.

Weapons need these working critical slots in one arm: axe, sword and claw, one
per 15 tons (rounded up); mace, one per 10 tons (rounded up); retractable blade, one
per 20 tons (rounded up) plus one; lance, one per 20 tons (rounded up); flail 4;
wrecking ball 5; chain whip 2; vibroblades 1, 2 or 4.

`disable <weapon>` powers down a Gauss weapon once it finishes recharging. You
must be piloting a running unit on a map. Comma-separated weapon numbers and
ranges are accepted. A powered-down weapon cannot fire and will not explode
when hit critically. It remains installed, keeps its ammunition, and stays
powered down after shutdown or restart. There is no cockpit command to power it
back up.

Clan ATM-3/6/9/12 launchers use 5/10/15 range bands, a four-hex minimum range,
and two damage per missile. Their built-in guidance adds 2 to the cluster roll
unless ECM interferes, and damage lands in five-point groups.
`atmrange <selection>` toggles Extended Range bins: one damage per missile at
9/18/27 hexes with a four-hex minimum. `atmexplosive <selection>` toggles High
Explosive bins: three damage per missile at 3/6/9 hexes with no minimum range.
A matching bin is required; toggle the command again to return to normal
ammunition. Templates use the `ExtendedRange` and `HighExplosive` flags. ATMs
fire directly only: they cannot fire indirectly or hotload (Total Warfare p. 141),
and they take no other special rounds.

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
pilot in the cockpit. `unitprefs MWSafety ON|OFF` changes the same setting.
Completed startup turns safety on; shutdown and interrupted startup retain its
setting. This preference concerns MechWarrior targets, which are not currently
supported combat targets. It does not change friendly-fire safety.

`ap <weapons>` is an alias for `armorpiercing <weapons>`. Both use the same
armor-piercing ammunition selection, including comma-separated weapon numbers
and ranges.
