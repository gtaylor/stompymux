---
title: btech.set_xy
type: docs
toc_hide: false
---

Places a live unit on a map at specified coordinates.

## Function

### Synopsis

```lua
btech.set_xy( unit, map, x, y, [z] )
```

### Arguments

`number unit`
: The unit dbref.

`number map`
: The destination map dbref.

`number x`
: The destination X coordinate.

`number y`
: The destination Y coordinate.

`number z`
: Optional altitude, defaulting to `0`.

### Returns

`boolean success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
