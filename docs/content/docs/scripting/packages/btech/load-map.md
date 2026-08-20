---
title: load_map
type: docs
toc_hide: false
---

Loads a map file into a map object and clears its units and map objects.

## Function

### Synopsis

```lua
btech.load_map( map, name, [clear] )
```

### Arguments

`number map`
: The map-object dbref.

`string name`
: The map file name.

`boolean clear`
: Optional compatibility argument; currently ignored.

### Returns

`boolean success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The optional `clear` argument is retained for compatibility; the current handler always clears units and map objects.

## See Also

- [`btech`](../)
