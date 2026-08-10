---
title: btech.part_cost
type: docs
toc_hide: false
---

Returns the configured cost of a part.

## Function

### Synopsis

```lua
btech.part_cost( part_name )
```

### Arguments

`string part_name`
: A recognized long or very-long part name.

### Returns

`number value`
: The numeric result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. This operation requires a build with advanced economy support.

## See Also

- [`btech`](../)
- [`btech.set_part_cost`](../set-part-cost/)
