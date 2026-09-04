---
draft: true
title: set_cost
type: docs
toc_hide: false
---

Sets the configured cost of a part.

## Function

### Synopsis

```lua
btech.parts.set_cost( part_name, cost )
```

### Arguments

`string part_name`
: A recognized long or very-long part name.

`integer cost`
: The cost parsed into native unsigned storage. The current native parser
  accepts a leading sign, so a negative input wraps into the unsigned range
  (for example, `-1` becomes the maximum unsigned value).

### Returns

`true success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. This operation requires a build with advanced economy support.

## See Also

- [`btech`](../../)
- [`btech.parts.cost`](../cost/)
