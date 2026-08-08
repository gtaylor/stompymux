---
title: btech.damages
linkTitle: btech.damages
type: docs
weight: 210
---

# `btech.damages`

Returns the formatted repair-job description for a live unit.

## Function

### Synopsis

```lua
btech.damages( unit )
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
