---
title: btech.xcode_value
type: docs
toc_hide: false
---

Reads a script-visible native field from a live special object.

## Function

### Synopsis

```lua
btech.xcode_value( object, name )
```

### Arguments

`number object`
: The special-object dbref.

`string name`
: The field name.

### Returns

`string result`
: The handler's serialized text result.

## Examples

```lua
local btech = require("btech")
local unit_id = btech.xcode_value(unit_dbref, "id")
```

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
- [`btech.set_xcode_value`](../set-xcode-value/)
- [`btech.xcode_value_ref`](../xcode-value-ref/)
