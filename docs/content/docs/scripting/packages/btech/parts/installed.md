---
title: installed
type: docs
toc_hide: false
---

Lists the parts installed on a live unit.

## Function

### Synopsis

```lua
btech.parts.installed( unit )
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

- [`btech`](../../)
- [`btech.parts.installed_ref`](../installed-ref/)
