---
title: btech.load_mech
linkTitle: btech.load_mech
type: docs
weight: 234
---

# `btech.load_mech`

Loads a unit template into a live unit object.

## Function

### Synopsis

```lua
btech.load_mech( unit, reference )
```

### Arguments

`number unit`
: The unit dbref.

`string reference`
: The unit template reference.

### Returns

`boolean success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
