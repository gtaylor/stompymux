---
title: load
type: docs
toc_hide: false
---

Loads map data from a named map file.

## Function

### Synopsis

```lua
btech.map.load( map, name )
```

### Arguments

`DbRef|Object map`
: The BattleTech map.

`string name`
: The map-file name.

### Returns

None.

## Notes

Loading replaces the map's terrain data and clears attached units and map objects.

## See Also

- [`btech`](../../)
- [`btech.map`](../)
