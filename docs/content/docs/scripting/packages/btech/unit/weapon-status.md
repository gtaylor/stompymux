---
title: weapon_status
type: docs
toc_hide: false
---

Returns serialized weapon status for a live unit or one section.

## Function

### Synopsis

```lua
btech.unit.weapon_status( unit, [section] )
```

### Arguments

`number unit`
: The unit dbref.

`string section`
: Optional full section name, matched without regard to case. Otherwise the
  legacy resolver uses a class-dependent one- or two-character prefix and may
  ignore trailing characters. It may be omitted, but explicitly passing `nil`
  raises an argument error.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
- [`btech.unit.weapon_status_ref`](../weapon-status-ref/)
