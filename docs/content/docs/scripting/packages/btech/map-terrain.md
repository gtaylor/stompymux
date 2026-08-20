---
title: map_terrain
type: docs
toc_hide: false
---

Returns the terrain code of a map hex.

## Function

### Synopsis

```lua
btech.map_terrain( map, x, y )
```

### Arguments

`number map`
: The map dbref.

`number x`
: The hex X coordinate.

`number y`
: The hex Y coordinate.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
