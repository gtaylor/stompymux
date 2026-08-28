---
title: type
type: docs
toc_hide: false
---

Returns the broad category of a part.

## Function

### Synopsis

```lua
btech.parts.type( part_name )
```

### Arguments

`string part_name`
: A recognized long or very-long part name.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
