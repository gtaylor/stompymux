---
title: btech.unit_parts
type: docs
toc_hide: false
---

Lists the parts installed on a live unit.

## Function

### Synopsis

```lua
btech.unit_parts( unit )
```

### Arguments

`number unit`
: The unit dbref.

### Returns

`table values`
: A flat array of converted legacy result tokens.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
- [`btech.unit_parts_ref`](../unit-parts-ref/)
