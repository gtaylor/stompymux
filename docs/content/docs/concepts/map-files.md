---
title: Map files
weight: 31
description: The TOML format for BattleTech battlefield maps
---

Map files describe a battlefield's terrain and environment. Stock maps live in
`game/maps`, the directory named by `database.map_database`.

## Files and names

Each map is a TOML document named `<name>.toml`. The name is what `LOADMAP`,
`SAVEMAP`, `@btech mapfile` and `@btech map-create` take, so `LOADMAP CC.top`
reads `game/maps/CC.top.toml`. Names may include subdirectories that already
exist under the map directory.

To generate a map from a biome, size, settlements and roads instead of drawing
one by hand, see [Map generation](../map-generation/).

`just check-maps` (part of `just checks`) parses every map in `game/maps` and
fails if any would not load.

## How a hex is built

Every hex is a stack of layers, following the BattleTech *Tactical Operations*
model of a base terrain plus terrain modifications:

| Layer | Holds | Grid or entry |
| --- | --- | --- |
| Ground | What the land is: clear, pavement, rough, ultra rough, rubble, ultra rubble, sand, tundra, swamp, magma crust, magma or heavy industrial. | `terrain` |
| Water | Water over clear ground, with its depth and whether it is still, rapids or a torrent. | `terrain` (`~`), `depth`, `flow` |
| Foliage | Light, heavy or ultra-heavy woods or jungle, or planted fields. | `foliage` |
| Route | A paved, gravel or dirt road, or a rail line, laid through the hex. | `route` |
| Structure | A building, wall or bridge, with a construction class and construction factor (CF). | `[[structures]]` |
| Condition | Ice, thin snow, deep snow or mud lying over the hex. | `condition` |
| Overlay | Fire or smoke. | `overlay` |

Layers combine wherever the manuals allow it: woods on sand, a road through
heavy jungle, ice over a road or over a river, deep snow on rough ground, or a
bridge over rapids. A hex is rejected when its layers contradict each other:

- Water lies only over clear ground, has no foliage or route under it, and
  only ice can lie on it. Rapids and torrents need at least depth 1.
- Nothing grows on pavement, heavy industrial ground or magma, and no road or
  condition lies on magma.
- A bridge needs water under it. A building or wall stands on dry ground with
  no foliage or route.

## Example

```toml
gravity = 100
temperature = -5
light = "twilight"
visibility = 20
wind = { direction = 270, speed = 15 }
flags = ["dark"]

terrain = '''
..%.~~..
.^^.~~__
'''
level = '''
00110000
02210000
'''
depth = '''
....23..
....34..
'''
flow = '''
....r...
........
'''
foliage = '''
.""....j
.....``.
'''
route = '''
......##
..d.....
'''
condition = '''
++......
.....-..
'''

[[structures]]
kind = "bridge"
class = "heavy"
height = 2
hexes = [[4, 0], [5, 0]]

[[structures]]
kind = "building"
class = "medium"
cf = 25
height = 3
hexes = [[7, 1]]

[[points_of_interest]]
type = "objective"
name = "Comms Tower"
x = 6
y = 1
elevation = 3
```

## Settings

| Key | Default | Meaning |
| --- | --- | --- |
| `gravity` | `100` | Gravity in percent of standard, 0 to 255. |
| `temperature` | `20` | Temperature in degrees Celsius, -128 to 127. |
| `light` | keep current | `"day"`, `"twilight"` or `"night"`. A new map starts in daylight. |
| `visibility` | keep current | Weather visibility in hexes, 0 to 60. A new map starts at 30. |
| `wind` | keep current | `{ direction, speed }`: the bearing the wind blows from, 0 to 359, and its strength, where 0 is calm. Wind carries smoke and spreads fire. A new map starts calm. |
| `flags` | keep current | Map flags by name, such as `"dark"` or `"underground"`. `flags = []` clears them. See `help @setmap` for the list. |

Keys marked "keep current" leave a loaded map's value alone when they are
absent, so reloading a map file does not reset light or wind that a scene
changed.

## Grids

Each grid is a TOML literal string (`'''`) with one character per hex and one
line per row. Every grid has the same width and height, which set the map's
size, up to 1000 by 1000. Heights use `0`-`9` then `a`-`z` for 10 to 35.

`terrain` and `level` are required, and `depth` is required when the map has
water. The other grids are optional. In every grid but `terrain` and `level`, `.`
means the hex has nothing in that layer.

| Grid | Character means |
| --- | --- |
| `terrain` | The ground, or `~` for water; see the table below. |
| `level` | Ground height. Water surfaces sit at this height; the water's bed is `depth` levels below it. |
| `depth` | Water depth below the surface, `0`-`9`, for every `~` hex. |
| `flow` | `r` for rapids and `t` for a torrent; still water is `.`. |
| `foliage` | `` ` `` light woods, `"` heavy woods, `W` ultra-heavy woods, `j` light jungle, `J` heavy jungle, `U` ultra-heavy jungle, `f` planted fields. |
| `route` | `#` paved road, `g` gravel road, `d` dirt road, `\|` rail. |
| `condition` | `-` ice, `*` thin snow, `+` deep snow, `,` mud. |
| `overlay` | Permanent fire (`&`) or smoke (`:`) over the hex. |

| Symbol | Ground |
| --- | --- |
| `.` | Clear |
| `_` | Pavement |
| `%` | Rough |
| `^` | Ultra rough |
| `;` | Rubble |
| `!` | Ultra rubble |
| `}` | Sand |
| `{` | Tundra |
| `w` | Swamp |
| `m` | Magma crust |
| `M` | Magma |
| `$` | Heavy industrial |
| `~` | Water |

## Structures

Buildings, walls and bridges are listed explicitly as `[[structures]]`
entries. Each gives:

| Key | Required | Meaning |
| --- | --- | --- |
| `kind` | yes | `"building"`, `"wall"` or `"bridge"`. |
| `height` | yes | Levels above the hex's ground: a building's roof, a wall's top or a bridge's deck, 1 to 35. |
| `class` | no | Construction class: `"light"` (CF up to 15), `"medium"` (40, the default), `"heavy"` (90) or `"hardened"` (150). |
| `cf` | no | Construction factor left, for a damaged structure. It defaults to the class's full value and may not exceed it. |
| `hexes` | yes | The `[x, y]` hexes it covers, counting from 0 at the top left. |

Every bridge hex must be water; the `terrain` grid shows the water beneath it.

Each structure hex tracks its own CF. Weapons fired at a hex wear it down, and
at 0 the hex collapses: a bridge falls into its water, and a building or wall
leaves rubble, or ultra rubble for heavy and hardened construction. Units
standing on it fall. The `indestructible_structures` map flag protects every
structure on the map.

## Points of interest

Points of interest mark hexes that scripts care about, such as objectives,
landing zones or supply caches. Units never see them: they do not appear on
any map display, sensor or terrain report, and they do not change the terrain.
Scripts read them with `btech.map.points_of_interest(map [, type])`, or from a
map file that is not loaded with `btech.map.inspect_file(name)`.

Each `[[points_of_interest]]` entry has these keys:

| Key | Required | Meaning |
| --- | --- | --- |
| `type` | yes | Category chosen by the map author. Any non-empty text; matching is case-sensitive, so `"Objective"` and `"objective"` are different types. |
| `name` | yes | Name chosen by the map author. Any non-empty text. |
| `x`, `y` | yes | The hex, counting from 0 at the top left. It must be on the map. |
| `elevation` | no | Height in levels relative to the hex's ground `level`, -128 to 127. Negative values are below the ground. |

Points keep the order they have in the file. Reloading a map replaces its
points of interest with the file's. Resizing a map drops the points that fall
off it.

## Saving

Fire and smoke are temporary conditions, not terrain. When a map is loaded,
each `&` and `:` in the `overlay` grid becomes a fire or smoke effect over that
hex's terrain that never burns out or drifts away.

`SAVEMAP <name>` writes the current map to `<name>.toml` in this format.
Permanent fire and smoke are saved in the `overlay` grid. Fire and smoke that will
burn out or drift away are not saved; those hexes save only the terrain underneath. Damaged structures save their remaining CF. Points of interest are saved. Mine
fields, landing zones and other map objects are kept in the database, not in map files.
