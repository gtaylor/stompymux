+++
title = "Line of Sight"
description = "How BattleTech units see and detect each other"
keywords = ["line of sight", "los", "terrain", "ice", "sensors", "perception", "visibility", "darkness", "light levels", "night", "dawn", "dusk", "probe", "radar", "ecm"]
article_tags = ["battletech"]
+++

# Line of Sight

Your unit perceives the battlefield automatically. There are no sensor modes to
pick; `sensor` shows how far and by what means you can currently perceive.

## A clear line

Everything starts with a clear line between you and the target, drawn by the
Total Warfare intervening terrain rule. A standing Mech sees from two levels above
its ground and a vehicle or prone Mech from one. Terrain along the line intervenes
when it is at least as high as both of you, or when it is next to one of you and at
least as high as that unit. Intervening hills, buildings and walls break the line;
bridges never do. Woods and smoke pile up instead: light woods and light smoke
count one, heavy woods and heavy smoke two and ultra-heavy woods three, and three
or more points break the line. Planted fields rise one level, so they intervene
only against low units; every two intervening hexes of crops cost one to hit and
six break the line. The target's own hex never intervenes, but woods or smoke in
it still cost their points to hit. Fire itself hides nothing; the smoke it gives
off does. A target under water is visible through fewer than six water hexes. The
map also has a maximum visibility beyond which nothing is seen.

## Partial cover

A standing Mech next to a hex one level above its own ground, lying along the
line to the attacker, has partial cover when the attacker is no higher than it
is. A Mech standing in water one level deep has partial cover from any height.
Partial cover costs one to hit and protects the legs: a hit rolled on a leg
strikes the cover instead. Hull-down units behind partial cover cost two more.

## Sensors and sight

- Sensors: within fifteen hexes you detect anything with a clear line, whatever
  the darkness or weather.
- Sight: beyond your sensors, the map's weather visibility sets how far you see.
  At night a target lit by a searchlight, fire or scenario lighting can be seen
  three times as far.

## Light

The map's light follows the Tactical Operations light conditions. Darkness adds
to the to-hit number of every weapon attack against a unit, and at night of
physical attacks too, whether you perceive the target by sensors or by sight:

| Light | Weapon | Physical | Lit target | Heat step |
| --- | --- | --- | --- | --- |
| Day | +0 | +0 | +0 | none |
| Dawn or dusk | +1 | +0 | +1 | 25 |
| Full moon night | +2 | +0 | +0 | 20 |
| Moonless night | +3 | +1 | +0 | 15 |
| Pitch black | +4 | +2 | +1 | 10 |

At night, a target lit by a searchlight, fire or scenario lighting, or one with
its own searchlight on, takes the "lit target" weapon modifier and no physical
modifier. Searchlights do not help at dawn or dusk. Hot Mechs stand out: a
weapon attack gets -1 for every full heat step of heat the target carries.

Contacts appear as soon as you perceive them and stay until nothing reaches them
any longer. Only hidden enemies take time to find: you cannot find them beyond
five hexes without a probe, and between three and five hexes you must search for
them.

## Special equipment

- Active probes (Beagle four hexes, Clan active probe and Watchdog five, Light
  three, Bloodhound eight) see through terrain, woods, smoke and darkness and
  reveal hidden units. A probe contact
  behind a hill shows as `p` in `contacts`: you can lock it, spot it for indirect
  fire and share it over C3, but you cannot fire at it directly or scan it.
- Radar on AntiAircraft units tracks airborne targets out to 180 hexes.
- Hostile ECM jams your sensors and probe, leaving you with sight alone.
- Stealth armor and null signature systems hide a unit from enemy sensors and
  from every probe except the Bloodhound. Stealth armor also jams its own
  sensors and probe.
- A damaged sensor system halves your sensor range; a destroyed one leaves you
  with sight alone.

Aim at a perceived target includes woods and smoke between you and in its hex,
crops along the line, partial cover, and the darkness penalty when it applies.
Aiming through a probe ignores woods, smoke and darkness; radar sees through
smoke and crops and gives a bonus against aircraft.

## Ice

An ice hex selected as a target is visible at the ice surface to an observer above
water when no other terrain blocks the path. Ice still counts as water terrain for
line-of-sight effects.
