---
title: btech.unit_fixable
type: docs
toc_hide: true
---

Tests whether a live unit can be repaired.

## Function

### Synopsis

```lua
btech.unit_fixable( unit )
```

### Arguments

`number unit`
: The unit dbref.

### Returns

`boolean result`
: Whether the condition is true.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
