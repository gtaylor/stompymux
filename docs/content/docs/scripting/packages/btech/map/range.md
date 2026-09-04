---
draft: true
title: range
type: docs
toc_hide: false
---

Calculates distance between units or map coordinates.

## Function

Calculates the range between two live units on the map.

### Synopsis

```lua
btech.map.range( map, unit_a, unit_b )
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
btech.map.range( map, unit, x, y )
btech.map.range( map, x, y, unit )
```

### Arguments

`number map`
: The map dbref.

`number unit`
: A unit dbref on the map.

`integer x`
: The hex X coordinate.

`integer y`
: The hex Y coordinate.

### Returns

`number range`
: The three-dimensional range from the unit to the hex.

## Function

Calculates the range between a live unit and a coordinate with an explicit
altitude.

### Synopsis

```lua
btech.map.range( map, unit, x, y, z )
btech.map.range( map, x, y, z, unit )
```

### Arguments

`number map`
: The map dbref.

`number unit`
: A unit dbref on the map.

`integer x`
: The X coordinate.

`integer y`
: The Y coordinate.

`integer z`
: The altitude.

### Returns

`number range`
: The three-dimensional range from the unit to the coordinate.

## Function

Calculates the range between two map hexes, using their terrain elevations.

### Synopsis

```lua
btech.map.range( map, x1, y1, x2, y2 )
```

### Arguments

`number map`
: The map dbref.

`integer x1, y1`
: The first hex coordinates.

`integer x2, y2`
: The second hex coordinates.

### Returns

`number range`
: The three-dimensional range between the hexes.

## Function

Calculates the range between two coordinates with explicit altitudes.

### Synopsis

```lua
btech.map.range( map, x1, y1, z1, x2, y2, z2 )
```

### Arguments

`number map`
: The map dbref.

`integer x1, y1, z1`
: The first coordinate and altitude.

`integer x2, y2, z2`
: The second coordinate and altitude.

### Returns

`number range`
: The three-dimensional range between the coordinates.

## Notes

This function is available only in a running Lua callback. Units must be on the
specified map and hex coordinates must be within its bounds. Invalid targets,
invalid arguments, and legacy error results raise a Lua error.

The mixed unit/coordinate forms have a legacy dispatch limitation. In the
four-argument form, an integer in the first overloaded position selects the
coordinate-first form, so the unit-first form requires a non-integer textual
reference such as `#42`. In the five-argument form, dispatch treats a leading
digit or `.` as numeric; numeric-looking unit references in either unit position
are therefore ambiguous. The signatures retain the canonical integer dbref type
despite this runtime defect.

## See Also

- [`btech`](../../)
