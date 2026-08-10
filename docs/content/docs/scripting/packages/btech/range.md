---
title: btech.range
type: docs
toc_hide: true
---

Calculates distance between units or map coordinates.

## Function

Calculates the range between two live units on the map.

### Synopsis

```lua
btech.range( map, unit_a, unit_b )
```

### Arguments

`number map`
: The map dbref.

`number unit_a`
: The first unit dbref.

`number unit_b`
: The second unit dbref.

### Returns

`number range`
: The three-dimensional range between the units.

## Function

Calculates the range between a live unit and a map hex, using the hex's
elevation.

### Synopsis

```lua
btech.range( map, unit, x, y )
btech.range( map, x, y, unit )
```

### Arguments

`number map`
: The map dbref.

`number unit`
: A unit dbref on the map.

`number x`
: The hex X coordinate.

`number y`
: The hex Y coordinate.

### Returns

`number range`
: The three-dimensional range from the unit to the hex.

## Function

Calculates the range between a live unit and a coordinate with an explicit
altitude.

### Synopsis

```lua
btech.range( map, unit, x, y, z )
btech.range( map, x, y, z, unit )
```

### Arguments

`number map`
: The map dbref.

`number unit`
: A unit dbref on the map.

`number x`
: The X coordinate.

`number y`
: The Y coordinate.

`number z`
: The altitude.

### Returns

`number range`
: The three-dimensional range from the unit to the coordinate.

## Function

Calculates the range between two map hexes, using their terrain elevations.

### Synopsis

```lua
btech.range( map, x1, y1, x2, y2 )
```

### Arguments

`number map`
: The map dbref.

`number x1, y1`
: The first hex coordinates.

`number x2, y2`
: The second hex coordinates.

### Returns

`number range`
: The three-dimensional range between the hexes.

## Function

Calculates the range between two coordinates with explicit altitudes.

### Synopsis

```lua
btech.range( map, x1, y1, z1, x2, y2, z2 )
```

### Arguments

`number map`
: The map dbref.

`number x1, y1, z1`
: The first coordinate and altitude.

`number x2, y2, z2`
: The second coordinate and altitude.

### Returns

`number range`
: The three-dimensional range between the coordinates.

## Notes

This function is available only in a running Lua callback. Units must be on the
specified map and hex coordinates must be within its bounds. Invalid targets,
invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
