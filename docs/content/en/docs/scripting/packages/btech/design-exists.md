---
title: btech.design_exists
linkTitle: btech.design_exists
type: docs
weight: 211
---

# `btech.design_exists`

Tests whether a unit template exists.

## Function

### Synopsis

```lua
btech.design_exists( reference )
```

### Arguments

`string reference`
: The unit template reference.

### Returns

`boolean result`
: Whether the condition is true.

## Examples

```lua
local btech = require("btech")

if btech.design_exists("AS7-D") then
  mux.notify(ctx.enactor, "The design is available.")
end
```

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
