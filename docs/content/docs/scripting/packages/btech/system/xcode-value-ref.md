---
title: xcode_value_ref
type: docs
toc_hide: false
---

Reads a script-visible native field from a unit template.

## Function

### Synopsis

```lua
btech.system.xcode_value_ref( reference, name )
```

### Arguments

`string reference`
: The unit template reference.

`string name`
: The field name.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The reference is resolved from the configured unit-template database.

## See Also

- [`btech`](../../)
- [`btech.system.xcode_value`](../xcode-value/)
