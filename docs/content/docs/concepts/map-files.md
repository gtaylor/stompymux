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
or ice, and `structure_height` when it has buildings or walls. Where a grid
does not apply to a hex, that hex is `.`.

| Grid | Character means |
| --- | --- |
| `terrain` | What the hex is; see the table below. |
| `level` | Ground height, `0`-`9` then `a`-`z` for 10 to 35. Water and ice surfaces sit at this height, which must be 0 for now. |
| `depth` | Water depth below the surface, `0`-`9`, for every `~` and `-` hex. |
| `structure_height` | Height of the building or wall above the ground, `0`-`9` then `a`-`z`, for every `@` and `=` hex. |

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
| `&` | Permanent fire over clear ground |
| `:` | Permanent smoke over clear ground |

## Bridges

Bridges are listed explicitly; the terrain grid shows the water beneath them.
Each `[[bridges]]` entry gives a `deck` height above the water surface and the
`[x, y]` hexes it covers, counting from 0 at the top left. Every bridge hex must
be water or ice.

## Saving

Fire and smoke are not terrain. When a map is loaded, each `&` and `:` hex becomes
clear ground with a fire or smoke effect over it that never burns out or drifts
away.

`SAVEMAP <name>` writes the current map to `<name>.toml` in this format.
Permanent fire and smoke over clear ground are saved as `&` and `:`. Fire and smoke
that will burn out or drift away, or that cover anything other than clear ground,
are not saved; those hexes save the terrain underneath. Mine fields, landing zones and other map objects are
kept in the database, not in map files.
