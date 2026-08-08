---
title: btech.mech_frequencies
type: docs
toc_hide: true
---

Lists the configured radio channels of a live unit.

## Function

### Synopsis

```lua
btech.mech_frequencies( unit )
```

### Arguments

`number unit`
: The unit dbref.

### Returns

`table values`
: A flat array of converted legacy result tokens.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The flattened array reflects the legacy channel serialization rather than structured channel records.

## See Also

- [`btech`](../)
