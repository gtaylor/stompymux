---
title: armor_status_ref
type: docs
toc_hide: false
---

Returns serialized armor values for one section of a unit template.

## Function

### Synopsis

```lua
btech.unit.armor_status_ref( reference, section )
```

### Arguments

`string reference`
: The unit template reference.

`string section`
: The section name.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The reference is resolved from the configured unit-template database.

## See Also

- [`btech`](../../)
- [`btech.unit.armor_status`](../armor-status/)
