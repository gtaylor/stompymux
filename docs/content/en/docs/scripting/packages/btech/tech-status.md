---
title: btech.tech_status
linkTitle: btech.tech_status
type: docs
weight: 265
---

# `btech.tech_status`

Returns formatted repair status for a live unit.

## Function

### Synopsis

```lua
btech.tech_status( unit )
```

### Arguments

`number unit`
: The unit dbref.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
