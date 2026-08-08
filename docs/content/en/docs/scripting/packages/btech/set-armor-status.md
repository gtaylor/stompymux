---
title: btech.set_armor_status
type: docs
toc_hide: true
---

Sets one armor-status field on a live unit section.

## Function

### Synopsis

```lua
btech.set_armor_status( unit, section, field, value )
```

### Arguments

`number unit`
: The unit dbref.

`string section`
: The section name.

`string field`
: The armor field to change.

`number value`
: The new value.

### Returns

`boolean success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
