---
title: set_tons
type: docs
toc_hide: false
---

Sets a live unit's tonnage and original weight.

## Function

### Synopsis

```lua
btech.unit.set_tons( unit, tons )
```

### Arguments

`number unit`
: The unit dbref.

`integer tons`
: The new tonnage.

### Returns

`true success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
