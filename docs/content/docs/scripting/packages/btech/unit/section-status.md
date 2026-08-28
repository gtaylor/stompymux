---
title: section_status
type: docs
toc_hide: false
---

Returns serialized status for one section of a live unit.

## Function

### Synopsis

```lua
btech.unit.section_status( unit, section )
```

### Arguments

`number unit`
: The unit dbref.

`string section`
: The section name.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
