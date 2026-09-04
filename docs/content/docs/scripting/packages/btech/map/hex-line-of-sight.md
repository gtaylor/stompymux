---
draft: true
title: hex_line_of_sight
type: docs
toc_hide: false
---

Tests whether a live unit has unobstructed line of sight to a map hex.

## Function

### Synopsis

```lua
btech.map.hex_line_of_sight( unit, x, y )
```

### Arguments

`number unit`
: The observing unit dbref.

`integer x`
: The target hex X coordinate.

`integer y`
: The target hex Y coordinate.

### Returns

`boolean result`
: Whether the condition is true.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

An overwater observer can trace LOS to an ice target hex at the ice surface;
ice still contributes its normal water-terrain effect to the LOS result.

## See Also

- [`btech`](../../)
- [`btech.map.unit_line_of_sight`](../unit-line-of-sight/)
