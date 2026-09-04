---
draft: true
title: load
type: docs
toc_hide: false
---

Loads a unit template into a live unit object.

## Function

### Synopsis

```lua
btech.unit.load( unit, reference )
```

### Arguments

`number unit`
: The unit dbref.

`string reference`
: The unit template reference.

### Returns

`true success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
