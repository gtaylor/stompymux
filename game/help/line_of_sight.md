+++
title = "Line of Sight"
description = "How BattleTech units see and detect each other"
keywords = ["line of sight", "los", "terrain", "ice", "sensors", "perception", "visibility", "darkness", "probe", "radar", "ecm"]
article_tags = ["battletech"]
+++

# Line of Sight

Your unit perceives the battlefield automatically. There are no sensor modes to
pick; `sensor` shows how far and by what means you can currently perceive.

## A clear line

Everything starts with a clear line between you and the target. Hills and
buildings, fire, smoke, three or more points of woods (light woods count one,
heavy woods two, not counting the target's own hex) and the cloud base all break
it. A target under water is visible through fewer than six water hexes. The map
also has a maximum visibility beyond which nothing is seen.

## Sensors and sight

- Sensors: within fifteen hexes you detect anything with a clear line, whatever
  the darkness or weather.
- Sight: beyond your sensors, the map's weather visibility sets how far you see.
  At night an unlit target costs +1 to hit. A target lit by a searchlight, fire or
  scenario lighting has no darkness penalty and can be seen three times as far.

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

Aim at a perceived target includes woods between you and in its hex, partial
cover, and the darkness penalty when it applies. Aiming through a probe ignores
woods and darkness; radar gives a bonus against aircraft.

## Ice

An ice hex selected as a target is visible at the ice surface to an observer above
water when no other terrain blocks the path. Ice still counts as water terrain for
line-of-sight effects.
