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

## Example

```toml
gravity = 100
temperature = 20
flags = ["dark"]

terrain = '''
..""~~#.
.^^"~~#.
'''
level = '''
00110000
02210000
'''
depth = '''
....23..
....34..
'''

[[bridges]]
deck = 2
hexes = [[4, 0], [5, 0]]

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
| `flags` | keep current | Map flags by name, such as `"dark"` or `"underground"`. When the key is absent, reloading a map keeps the flags it already has; `flags = []` clears them. See `help @setmap` for the list. |

## Grids

Each grid is a TOML literal string (`'''`) with one character per hex and one
line per row. Every grid has the same width and height, which set the map's
size, up to 1000 by 1000.

`terrain` and `level` are required. `depth` is required when the map has water
or ice, and `structure_height` when it has buildings or walls. `overlay` is
optional. Where a grid does not apply to a hex, that hex is `.`.

| Grid | Character means |
| --- | --- |
| `terrain` | What the hex is; see the table below. |
| `level` | Ground height, `0`-`9` then `a`-`z` for 10 to 35. Water and ice surfaces, and the water under a bridge, sit at this height; the water's bed is `depth` levels below it. |
| `depth` | Water depth below the surface, `0`-`9`, for every `~` and `-` hex. |
| `structure_height` | Height of the building or wall above the ground, `0`-`9` then `a`-`z`, for every `@` and `=` hex. |
| `overlay` | Permanent fire (`&`) or smoke (`:`) over the hex, whatever its terrain. |

| Symbol | Terrain |
| --- | --- |
| `.` | Clear ground |
| `#` | Road |
| `%` | Rough ground |
| `^` | Mountains |
| `+` | Snow |
| `}` | Sand |
| `` ` `` | Light woods |
| `"` | Heavy woods |
| `~` | Water |
| `-` | Ice over water |
| `@` | Building |
| `=` | Wall |

## Bridges

Bridges are listed explicitly; the terrain grid shows the water beneath them.
Each `[[bridges]]` entry gives a `deck` height above the water surface and the
`[x, y]` hexes it covers, counting from 0 at the top left. Every bridge hex must
be water or ice.

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

Fire and smoke are not terrain. When a map is loaded, each `&` and `:` in the
`overlay` grid becomes a fire or smoke effect over that hex's terrain that never
burns out or drifts away.

`SAVEMAP <name>` writes the current map to `<name>.toml` in this format.
Permanent fire and smoke are saved in the `overlay` grid. Fire and smoke that will
burn out or drift away are not saved; those hexes save only the terrain underneath. Points of interest are saved. Mine
fields, landing zones and other map objects are kept in the database, not in map files.
