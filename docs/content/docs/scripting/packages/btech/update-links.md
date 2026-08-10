---
title: btech.update_links
type: docs
toc_hide: true
---

Recursively updates links associated with a map.

## Function

### Synopsis

```lua
btech.update_links( map )
```

### Arguments

`number map`
: The map dbref.

### Returns

`boolean success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
