---
title: place_unit
type: docs
toc_hide: false
---

Places a live unit at a position on a map.

## Function

### Synopsis

```lua
btech.map.place_unit( unit, map, position )
```

### Arguments

`DbRef|Object unit`
: The live unit.

`DbRef|Object map`
: The destination BattleTech map.

`BtechPosition position`
: The zero-based `x` and `y` coordinates and optional `z` altitude. When
  provided, `z` must be an integer from 0 through 10000.

### Returns

None.

## Notes

If the unit is carrying or towing another live unit, that unit is also moved to
the destination map at the same coordinates and altitude.

## See Also

- [`btech`](../../)
- [`btech.map`](../)
