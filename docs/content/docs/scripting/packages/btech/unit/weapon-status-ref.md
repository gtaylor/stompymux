---
title: weapon_status_ref
type: docs
toc_hide: false
---

Returns serialized weapon status for a unit template or one section.

## Function

### Synopsis

```lua
btech.unit.weapon_status_ref( reference, [section] )
```

### Arguments

`string reference`
: The unit template reference.

`string section`
: Optional full section name, matched without regard to case. Otherwise the
  legacy resolver uses a class-dependent one- or two-character prefix and may
  ignore trailing characters. It may be omitted, but explicitly passing `nil`
  raises an argument error.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The reference is resolved from the configured unit-template database.

## See Also

- [`btech`](../../)
- [`btech.unit.weapon_status`](../weapon-status/)
