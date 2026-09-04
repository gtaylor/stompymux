---
draft: true
title: armor_status_ref
type: docs
toc_hide: false
---

Returns serialized armor values for one section of a unit template or aggregate
armor and internal-structure totals.

## Function

### Synopsis

```lua
btech.unit.armor_status_ref( reference, section )
```

### Arguments

`string reference`
: The unit template reference.

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

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The reference is resolved from the configured unit-template database.

## See Also

- [`btech`](../../)
- [`btech.unit.armor_status`](../armor-status/)
