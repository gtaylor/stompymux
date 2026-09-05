---
title: units
type: docs
toc_hide: false
---

Lists the live units on a map.

## Function

### Synopsis

```lua
btech.map.units( map, filter )
```

### Arguments

`DbRef|Object map`
: The BattleTech map.

`table|nil filter`
: An optional `{ origin = { x = x, y = y }, range = range }` filter. The
  origin must be a valid map hex, and `range` must be a non-negative number.

### Returns

`Object[] units`
: The matching unit handles.

## See Also

- [`btech`](../../)
- [`btech.map`](../)
