---
draft: true
title: crit_status
type: docs
toc_hide: false
---

Returns serialized critical-slot status for one section of a live unit.

## Function

### Synopsis

```lua
btech.unit.crit_status( unit, section )
```

### Arguments

`number unit`
: The unit dbref.

`string section`
: A full section name, matched without regard to case. Otherwise the legacy
  resolver uses a class-dependent one- or two-character prefix and may ignore
  trailing characters.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
- [`btech.unit.crit_status_ref`](../crit-status-ref/)
