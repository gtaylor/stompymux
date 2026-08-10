---
title: btech.set_max_speed
type: docs
toc_hide: true
---

Sets a live unit's maximum speed and corrects its current speed.

## Function

### Synopsis

```lua
btech.set_max_speed( unit, speed )
```

### Arguments

`number unit`
: The unit dbref.

`number speed`
: The new maximum speed.

### Returns

`boolean success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
