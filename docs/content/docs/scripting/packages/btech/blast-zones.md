---
title: btech.blast_zones
type: docs
toc_hide: true
---

Lists blast-zone coordinates and radii on a map.

## Function

### Synopsis

```lua
btech.blast_zones( map )
```

### Arguments

`number map`
: The map dbref.

### Returns

`table values`
: A flat array of repeating `x`, `y`, and radius numbers.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
