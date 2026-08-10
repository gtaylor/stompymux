---
title: btech.payload_ref
type: docs
toc_hide: false
---

Returns the weapon and ammunition payload of a unit template.

## Function

### Synopsis

```lua
btech.payload_ref( reference )
```

### Arguments

`string reference`
: The unit template reference.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The reference is resolved from the configured unit-template database.

## See Also

- [`btech`](../)
