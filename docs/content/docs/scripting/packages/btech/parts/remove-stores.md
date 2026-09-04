---
draft: true
title: remove_stores
type: docs
toc_hide: false
---

Removes a quantity of a part from an object's stores.

## Function

### Synopsis

```lua
btech.parts.remove_stores( target, part_name, quantity )
```

### Arguments

`number target`
: The stores-bearing object dbref.

`string part_name`
: A recognized part name.

`integer quantity`
: The quantity to remove.

### Returns

`true success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
