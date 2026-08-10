---
title: btech.crit_slot
type: docs
toc_hide: true
---

Describes one critical slot on a live unit.

## Function

### Synopsis

```lua
btech.crit_slot( unit, section, slot, [name_type] )
```

### Arguments

`number unit`
: The unit dbref.

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

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
