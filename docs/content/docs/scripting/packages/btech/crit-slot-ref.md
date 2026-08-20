---
title: crit_slot_ref
type: docs
toc_hide: false
---

Describes one critical slot in a unit template.

## Function

### Synopsis

```lua
btech.crit_slot_ref( reference, section, slot, [name_type] )
```

### Arguments

`string reference`
: The unit template reference.

`string section`
: The section name.

`number slot`
: The critical-slot number.

`number name_type`
: Optional naming mode: `0` for template names or `1` for repair-part names.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The reference is resolved from the configured unit-template database.

## See Also

- [`btech`](../)
