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
light = "dusk"
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
`..`....
'''
route = '''
......##
..d.....
'''
condition = '''
++......
.....-..
'''
overlay = '''
..&.....
...:....
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

[[regions]]
type = "deployment"
name = "West LZ"
corners = [[0, 0], [2, 0], [2, 1], [0, 1]]
```

## Settings

| Key | Default | Meaning |
| --- | --- | --- |
| `gravity` | `100` | Gravity in percent of standard, 0 to 255. Missile and ballistic fire takes +1 to hit for every full 20 points away from 100. |
| `temperature` | `20` | Temperature in degrees Celsius, -128 to 127. Each started 10 degrees beyond -30 or 50 costs Mechs a point of cooling (or gives them one) and vehicles a cruising MP. |
| `light` | keep current | `"day"`, `"dawn"`, `"dusk"`, `"full_moon_night"`, `"moonless_night"` or `"pitch_black"`; see [Light](#light). A new map starts in daylight. |
| `visibility` | keep current | Weather visibility in hexes, 0 to 60. A new map starts at 30. |
| `wind` | keep current | `{ direction, speed }`: the bearing the wind blows from, 0 to 359, and its speed in km/h, where 0 is calm. Wind carries smoke and spreads fire. From 62 km/h it spoils missile and then ballistic fire, and from 75 km/h it makes piloting harder; see [Wind](#wind). A new map starts calm. |
| `flags` | keep current | Map flags by name, such as `"dark"` or `"underground"`. `flags = []` clears them. See `help @setmap` for the list. |

Keys marked "keep current" leave a loaded map's value alone when they are
absent, so reloading a map file does not reset light or wind that a scene
changed.

### Light

Light follows the light conditions of Tactical Operations. Darkness adds to
the to-hit number of weapon attacks against units, and at night of physical
attacks too:

| Light | Weapon | Physical | Lit target | Heat step |
| --- | --- | --- | --- | --- |
| `day` | +0 | +0 | +0 | none |
| `dawn`, `dusk` | +1 | +0 | +1 | 25 |
| `full_moon_night` | +2 | +0 | +0 | 20 |
| `moonless_night` | +3 | +1 | +0 | 15 |
| `pitch_black` | +4 | +2 | +1 | 10 |

At night a target lit by a searchlight, fire or other light, or one with its
own searchlight on, takes the "lit target" weapon modifier and no physical
modifier; searchlights do not help at dawn or dusk. A Mech target is also
easier to hit by one for every full heat step of heat it carries. At night a
lit target can be seen three times as far, and automatic searchlights switch
on. Glare plays as a full moon night and a solar flare as
a moonless one.

### Wind

Wind speed falls into the wind weather conditions of Tactical Operations,
using the Beaufort scale for gales and storms and the Fujita scale for
tornadoes. Stronger wind adds to the to-hit number of missile and direct-fire
ballistic attacks and to piloting checks:

| Speed (km/h) | Category | Missile | Ballistic | Energy | Mech piloting | Hover/VTOL piloting |
| --- | --- | --- | --- | --- | --- | --- |
| 0-49 | calm | +0 | +0 | +0 | +0 | +0 |
| 50-61 | light gale | +0 | +0 | +0 | +0 | +0 |
| 62-74 | moderate gale | +1 | +0 | +0 | +0 | +0 |
| 75-88 | strong gale | +2 | +1 | +0 | +1 | +2 |
| 89-116 | storm | +3 | +2 | +0 | +3 | +3 |
| 117-332 | tornado (F1-F3) | cannot fire | +3 | +2 | +3 | +3 |
| 333+ | tornado (F4+) | cannot fire | cannot fire | +3 | +5 | +5 |

Tracked and wheeled vehicles feel only tornadoes, which add +3 to their
piloting checks (+5 in an F4 tornado).

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

In Mappy, the **Points** tool (P) edits them. Click an empty hex to add a point
there, click a point to select it (clicking again cycles through the points
that share a hex), and drag to move it. The side panel edits the selected
point's type, name and elevation, lists every point on the map, and deletes the
selected point (or press Delete). Markers show every point over the map
whichever tool is selected, and hovering a hex lists its points.

## Regions

Regions are named areas of hexes that scripts care about, such as deployment
zones, objectives that cover several hexes, or areas units must hold. Like
points of interest, units never see them and they do not change the terrain.

A region is outlined by its corner hexes, listed in order. It holds every hex
the outline passes through, tracing straight lines between the centers of
consecutive corners and from the last corner back to the first, plus every hex
whose center lies inside the outline. One corner makes a single-hex region and
two make a line of hexes. Where an outline crosses itself, the areas it wraps
around an even number of times are left out, though the outline itself always
counts. A region never reaches past the rows and columns its corners span.

Each `[[regions]]` entry has these keys:

| Key | Required | Meaning |
| --- | --- | --- |
| `type` | yes | Category chosen by the map author. Any non-empty text; matching is case-sensitive. |
| `name` | yes | Name chosen by the map author. Any non-empty text. |
| `corners` | yes | One or more `[x, y]` corner hexes in outline order, counting from 0 at the top left. Each must be on the map. |

Regions keep the order they have in the file, and each region keeps its
corners in order. Reloading a map replaces its regions with the file's.
Resizing a map drops the corners that fall off it, so the outline runs through
the corners that remain, and drops a region that has none left.

In Mappy, the **Regions** tool (G) edits them. With no region selected, click a
region's hex to select it, or click anywhere else to start a new region there.
With a region selected, click one of its corners to select that corner and drag
to move it, or click anywhere else to add a corner after the selected one.
Delete removes the selected corner, and the region with its last corner; Escape
or **Done** lets go of the region. The side panel edits the selected region's
name and type, lists its corners and every region on the map, and counts their
hexes. Region hexes are tinted and outlined over the map whichever tool is
selected, the selected region in its own color with numbered corners.

## Saving

Fire and smoke are temporary conditions, not terrain. When a map is loaded,
each `&` and `:` in the `overlay` grid becomes a fire or smoke effect over that
hex's terrain that never burns out or drifts away.

`SAVEMAP <name>` writes the current map to `<name>.toml` in this format.
Permanent fire and smoke are saved in the `overlay` grid. Fire and smoke that will
burn out or drift away are not saved; those hexes save only the terrain underneath. Damaged structures save their remaining CF. Points of interest and regions are saved. Mine
fields, landing zones and other map objects are kept in the database, not in map files.
