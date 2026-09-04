---
draft: true
title: armor_status
type: docs
toc_hide: false
---

Returns serialized armor values for one section of a live unit or aggregate
armor and internal-structure totals.

## Function

### Synopsis

```lua
btech.unit.armor_status( unit, section )
```

### Arguments

`number unit`
: The unit dbref.

`string section`
: A full section name, matched without regard to case. Otherwise the legacy
  resolver uses a class-dependent one- or two-character prefix and may ignore
  trailing characters. The special selector `all` returns aggregate totals and
  must be exactly lowercase.

### Returns

`string result`
: Per-section armor, internal, and rear-armor values, or aggregate current and
  original armor/internal totals for `all`.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
- [`btech.unit.armor_status_ref`](../armor-status-ref/)
