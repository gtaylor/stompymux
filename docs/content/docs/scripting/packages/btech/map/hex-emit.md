---
title: hex_emit
type: docs
toc_hide: false
---

Broadcasts a message from one map hex.

## Function

### Synopsis

```lua
btech.map.hex_emit( map, x, y, message )
```

### Arguments

`number map`
: The map dbref.

`number x`
: The hex X coordinate.

`number y`
: The hex Y coordinate.

`string message`
: A non-empty message.

### Returns

`boolean success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
