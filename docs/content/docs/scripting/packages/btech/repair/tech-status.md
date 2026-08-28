---
title: tech_status
type: docs
toc_hide: false
---

Returns formatted repair status for a live unit.

## Function

### Synopsis

```lua
btech.repair.tech_status( unit )
```

### Arguments

`number unit`
: The unit dbref.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
