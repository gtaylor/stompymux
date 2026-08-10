---
title: btech.map_units
type: docs
toc_hide: false
---

Lists all units on a map or those within a 2D or 3D range.

## Function

Lists every live unit on a map.

### Synopsis

```lua
btech.map_units( map )
```

### Arguments

`number map`
: The map dbref.

### Returns

`table units`
: An array of unit dbrefs.

## Function

Lists units within a two-dimensional range of a map coordinate.

### Synopsis

```lua
btech.map_units( map, x, y, range )
```

### Arguments

`number map`
: The map dbref.

`number x, y`
: The origin hex coordinates.

`number range`
: A non-negative range.

### Returns

`table units`
: An array of unit dbrefs.

## Function

Lists units within a three-dimensional range of a map coordinate.

### Synopsis

```lua
btech.map_units( map, x, y, z, range )
```

### Arguments

`number map`
: The map dbref.

`number x, y, z`
: The origin hex coordinates and altitude.

`number range`
: A non-negative range.

### Returns

`table units`
: An array of unit dbrefs.

## Examples

```lua
local btech = require("btech")

for _, unit in ipairs(btech.map_units(map_dbref)) do
  mux.notify(ctx.enactor, mux.object(unit).name)
end
```

## Notes

This function is available only in a running Lua callback. Invalid targets,
out-of-bounds coordinates, negative ranges, and legacy error results raise a
Lua error. Unit dbrefs are converted from the legacy `#123` representation to
Lua numbers.

## See Also

- [`btech`](../)
- [`btech.range`](../range/)
