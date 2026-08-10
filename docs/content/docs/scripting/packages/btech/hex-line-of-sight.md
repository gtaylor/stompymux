---
title: btech.hex_line_of_sight
type: docs
toc_hide: true
---

Tests whether a live unit has unobstructed line of sight to a map hex.

## Function

### Synopsis

```lua
btech.hex_line_of_sight( unit, x, y )
```

### Arguments

`number unit`
: The observing unit dbref.

`number x`
: The target hex X coordinate.

`number y`
: The target hex Y coordinate.

### Returns

`boolean result`
: Whether the condition is true.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
- [`btech.mech_line_of_sight`](../mech-line-of-sight/)
