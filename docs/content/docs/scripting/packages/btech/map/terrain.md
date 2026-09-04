---
draft: true
title: terrain
type: docs
toc_hide: false
---

Returns the terrain code of a map hex.

## Function

### Synopsis

```lua
btech.map.terrain( map, x, y )
```

### Arguments

`number map`
: The map dbref.

`integer x`
: The hex X coordinate.

`integer y`
: The hex Y coordinate.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
