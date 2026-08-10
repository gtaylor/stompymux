---
title: btech.map_elevation
type: docs
toc_hide: true
---

Returns the elevation of a map hex.

## Function

### Synopsis

```lua
btech.map_elevation( map, x, y )
```

### Arguments

`number map`
: The map dbref.

`number x`
: The hex X coordinate.

`number y`
: The hex Y coordinate.

### Returns

`number value`
: The numeric result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
