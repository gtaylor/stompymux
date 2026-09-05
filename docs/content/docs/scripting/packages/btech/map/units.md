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
: An optional `{ origin = { x, y }, range }` filter.

### Returns

`Object[] units`
: The matching unit handles.

## See Also

- [`btech`](../../)
- [`btech.map`](../)
