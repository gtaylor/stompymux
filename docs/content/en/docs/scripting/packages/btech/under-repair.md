---
title: btech.under_repair
linkTitle: btech.under_repair
type: docs
weight: 269
---

# `btech.under_repair`

Tests whether a live unit has an active repair event.

## Function

### Synopsis

```lua
btech.under_repair( unit )
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
