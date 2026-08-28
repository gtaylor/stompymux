---
title: installed_ref
type: docs
toc_hide: false
---

Lists the parts installed in a unit template.

## Function

### Synopsis

```lua
btech.parts.installed_ref( reference )
```

### Arguments

`string reference`
: The unit template reference.

### Returns

`table values`
: A flat array of converted legacy result tokens.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The reference is resolved from the configured unit-template database.

## See Also

- [`btech`](../../)
- [`btech.parts.installed`](../installed/)
