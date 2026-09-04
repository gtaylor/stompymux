---
title: update_links
type: docs
toc_hide: false
---

Recursively updates links associated with a map.

## Function

### Synopsis

```lua
btech.map.update_links( map )
```

### Arguments

`number map`
: The map dbref.

### Returns

`true success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
