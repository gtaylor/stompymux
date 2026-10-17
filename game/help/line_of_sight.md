+++
title = "Line of Sight"
description = "How BattleTech units see and detect each other"
keywords = ["line of sight", "los", "terrain", "ice", "sensors", "perception", "visibility", "darkness", "light levels", "night", "dawn", "dusk", "weather", "fog", "probe", "infrared", "magscan", "seismic", "radar", "ecm", "spotting"]
article_tags = ["battletech"]
+++

# Line of Sight

Units perceive the battlefield by sight and by their sensor suite, following the
Total Warfare line-of-sight rules and the Tactical Operations double-blind
rules. Perception is automatic; `sensor` shows how far and by what means your
unit can perceive right now.

## A clear line

Everything starts with a clear line between you and the target, drawn by the
Total Warfare intervening terrain rule. A standing Mech sees from two levels above
its ground and a vehicle, a prone Mech or a kneeling biped from one. Terrain along
the line intervenes when it is at least as high as both of you, or when it is next
to one of you and at least as high as that unit. Intervening hills, buildings and
walls break the line; bridges never do, and units never do. Woods and smoke pile
up instead: light woods and light smoke count one, heavy woods and heavy smoke
two and ultra-heavy woods three, and three or more points break the line. Planted
fields rise one level, so they intervene only against low units; every two
intervening hexes of crops cost one to hit and six break the line. The target's
own hex never intervenes, but woods or smoke in it still cost their points to
hit. Fire itself hides nothing; the smoke it gives off does. A target under water
is visible through fewer than six water hexes. When a hex rather than a unit is
the target, the hex counts as one level lower, so a line can reach a hex it could
not reach a unit in.

## Partial cover

A standing Mech next to a hex one level above its own ground, lying along the
line to the attacker, has partial cover when the attacker is no higher than it
is. A Mech standing in water one level deep has partial cover from any height.
Partial cover costs one to hit and protects the legs: a hit rolled on a leg
strikes the cover instead. Hull-down units behind partial cover cost two more.

## Buildings

A unit inside a building hex, below its roof, is seen and attacked by the level
it stands on; its own hex never blocks the line, though other building hexes
do. A standing Mech inside one level below the roof has partial cover from any
height, and the building takes a tenth of its construction factor off every hit
grouping fired at a unit inside it. See `help piloting` for entering buildings.

## Sight

Sight is automatic. A target with a clear line is seen when it is within the
visual range the light and the weather allow, whichever is shorter. Visual range
counts hexes on the map, so height above or below the target never adds to it:

| Light | Range | Weather | Range |
| --- | --- | --- | --- |
| Day | 60 | Clear, lightning storm | 60 |
| Dawn or dusk | 15 | Light fog, rain, snow or hail | 30 |
| Full moon night | 10 | Moderate rain or snow | 20 |
| Moonless night | 5 | Heavy rain, snow flurries | 15 |
| Pitch black | 3 | Gusting rain, downpour, heavy snow, sleet, heavy hail, ice storm, blowing sand | 10 |
| | | Heavy fog | 5 |

Every point of smoke along the line takes a hex off that reach. At night your own
searchlight lets you see thirty hexes, and a target lit by a searchlight, by fire
or by scenario lighting is seen out to forty-five. These stand in for the light
only: fog, rain, snow and the like still limit sight, so at night in heavy fog a
lit target is seen five hexes off. Seeing a unit identifies it.

## Sensors

When sight fails, your sensors may still find the target. At the start of every
thirty-second turn each running unit rolls one sensor sweep: 7 or 8 finds
targets in the short band, 5 or 6 in the medium band, 2 to 4 in the long band,
and 9 or more finds nothing until the next sweep. The roll is tried against
every system the unit carries, a working active probe first and then the
built-in sensors, and the target is found if any system's band covers it. A
sensor contact tells you where something is, not what it is.

| Sensor | Mech | Vehicle | Blocked by | Notes |
| --- | --- | --- | --- | --- |
| Infrared | 10/20/30 | 9/18/27 | Hills | Heavy woods -1, ultra-heavy -2, buildings -1, hardened buildings -2 per hex; a hot Mech is found one hex farther per heat movement penalty, plus one for a unit in a burning hex or a stationary unit burning with inferno jelly |
| Magscan | 10/20/30 | 9/18/27 | Buildings, hardened included | 80-100 ton targets +1 hex, heavier +2; units under 20 tons and heavy industrial hexes hide |
| Radar | 8/16/24 | 6/12/18 | Hills, buildings, hardened included | Woods and buildings shorten it like infrared |
| Seismic | 2/4/6 | 1/2/3 | Nothing | Finds only units moving on the ground |
| Active probe | Probe ranges | Probe ranges | Nothing | Beagle 12/24/36, Clan 15/30/45, Light and Watchdog 9/18/27, Bloodhound 16/32/48 |

The surface of water stops every sensor but magscan. Guardian ECM, Angel ECM,
stealth armor and null signature systems add to the sweep roll of radars and
probes, often beyond the band that would have found the target, and every hostile
ECM suite around you or the target adds its own penalty; infrared, magscan
and seismic sensors ignore them. A Beagle slot is the Clan probe when the probe itself is
Clan-built: its slot says `CL.`, or it says neither `IS.` nor `CL.` on a Clan design. The
probe's own technology, not the chassis's, sets its brackets, reach and ECM
penalties. Damaged sensors leave you with sight alone. Electromagnetic interference
renders active probes useless but leaves the built-in sensors working (Tactical
Operations: Advanced Rules p. 53).

## Light

The map's light follows the Tactical Operations light conditions. Darkness adds
to the to-hit number of every weapon attack against a unit, and at night of
physical attacks too, however you perceive the target:

| Light | Weapon | Physical | Lit target | Heat step |
| --- | --- | --- | --- | --- |
| Day | +0 | +0 | +0 | none |
| Dawn or dusk | +1 | +0 | +1 | 25 |
| Full moon night | +2 | +0 | +0 | 20 |
| Moonless night | +3 | +1 | +0 | 15 |
| Pitch black | +4 | +2 | +1 | 10 |

At night, a target lit by a searchlight or scenario lighting, or one with its own
searchlight on, takes the "lit target" weapon modifier and no physical modifier.
A target lit by fire takes two less than the darkness modifier. Searchlights do
not help at dawn or dusk. Hot Mechs stand out: a weapon attack gets -1 for every
full heat step of heat the target carries. A Mech with laser heat sinks glows when
it makes heat: in a turn it moves, jumps or fires, weapon attacks against it take
one less darkness modifier, and with any heat on its scale no darkness modifier
applies to any attack on it (Tactical Operations: Advanced Units & Equipment
p. 129). The glow does not light it for sight.

## Weather

Weather adds its own to-hit modifiers by weapon class: moderate and heavy rain
and snow +1 to everything, a torrential downpour +2, gusting rain +2 to
ballistic and +3 to missile fire, snow flurries, sleet and lightning storms +1
(+2 to missiles), heavy fog +1 to energy weapons, light hail and ice storms +1
to missiles, heavy hail +1 to ballistic and +2 to missile fire, and blowing
sand +1 to energy and missile fire. Fog costs ground units one movement point
per hex (two in heavy fog), and heavy rain, heavy snow, gusting rain, downpours
and heavy hail add to piloting and driving rolls.

## Spotting for your side

Contacts appear as soon as a channel reaches them and stay until nothing does.
Anything a running unit on your side has in its contact list is a target for the
whole side: it appears in your `contacts` without a detection letter when you do
not perceive it yourself, and you may fire at it as long as you have your own
line of fire. C3 and C3i networks add their shared aiming range on top of that.

Aim at a target includes woods and smoke between you and in its hex, crops along
the line, partial cover and the darkness penalty when it applies. A working active
probe within its reach lets you ignore one point of woods, unless hostile ECM jams
it or shields the target (see `help piloting`). A shot at a hex
pays for the woods, smoke and crops between you and it, not for its own, and takes
-4 because a hex never moves.

## Ice

An ice hex selected as a target is visible at the ice surface to an observer above
water when no other terrain blocks the path. Ice still counts as water terrain for
line-of-sight effects.
