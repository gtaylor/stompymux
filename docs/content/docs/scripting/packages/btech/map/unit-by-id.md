---
title: unit_by_id
type: docs
toc_hide: false
---

Finds a unit by its tactical ID.

## Function

### Synopsis

```lua
btech.map.unit_by_id( origin, id )
```

### Arguments

`DbRef|Object origin`
: A BattleTech map or a unit on the map.

`string id`
: An exact two-character ASCII tactical ID.

### Returns

`Object|nil unit`
: The matching unit, or `nil` when none exists.

## See Also

- [`btech`](../../)
- [`btech.map`](../)
