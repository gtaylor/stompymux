---
title: weapon_status
type: docs
toc_hide: false
---

Returns serialized weapon status for a live unit or one section.

## Function

### Synopsis

```lua
btech.weapon_status( unit, [section] )
```

### Arguments

`number unit`
: The unit dbref.

`string section`
: Optional section name.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
- [`btech.weapon_status_ref`](../weapon-status-ref/)
