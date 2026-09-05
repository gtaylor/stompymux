---
title: emit
type: docs
toc_hide: false
---

Emits a message to units on a map.

## Function

### Synopsis

```lua
btech.map.emit( map, message, options )
```

### Arguments

`DbRef|Object map`
: The BattleTech map.

`string message`
: The non-empty message to emit.

`BtechMapEmitOptions|nil options`
: Optional audience, origin, and range controls.

### Returns

None.

## Notes

The `all` audience forbids `origin` and `range`. The `range` audience requires
both and requires a non-negative `range`. A range emission uses two-dimensional
distance when `origin.z` is omitted and three-dimensional distance when it is
provided. The `line_of_sight` audience requires `origin`, forbids `origin.z`,
and forbids `range`.

## See Also

- [`btech`](../../)
- [`btech.map`](../)
