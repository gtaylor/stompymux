---
draft: true
title: set_xy
type: docs
toc_hide: false
---

Places a live unit on a map at specified coordinates.

## Function

### Synopsis

```lua
btech.map.set_xy( unit, map, x, y, [z] )
```

### Arguments

`number unit`
: The unit dbref.

`number map`
: The destination map dbref.

`integer x`
: The destination X coordinate.

`integer y`
: The destination Y coordinate.

`integer z`
: Optional altitude, defaulting to `0` when omitted. Explicitly passing `nil`
  raises an argument error.

### Returns

`true success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
