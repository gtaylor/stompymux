---
title: btech.set_xcode_value
type: docs
toc_hide: false
---

Writes a script-writable native field on a live special object.

## Function

### Synopsis

```lua
btech.set_xcode_value( object, name, value )
```

### Arguments

`number object`
: The special-object dbref.

`string name`
: The writable field name.

`string or number value`
: The new value, converted according to the field type.

### Returns

`boolean success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
