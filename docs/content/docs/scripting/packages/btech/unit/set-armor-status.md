---
draft: true
title: set_armor_status
type: docs
toc_hide: false
---

Sets one armor-status field on a live unit section.

## Function

### Synopsis

```lua
btech.unit.set_armor_status( unit, section, field, value )
```

### Arguments

`number unit`
: The unit dbref.

`string section`
: A full section name, matched without regard to case. Otherwise the legacy
  resolver uses a class-dependent one- or two-character prefix and may ignore
  trailing characters.

`integer field`
: `0` changes current armor, `1` internal structure, and `2` rear armor.

`integer value`
: The new value, from `0` through `255` inclusive.

### Returns

`true success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
