---
title: line_of_sight
type: docs
toc_hide: false
---

Tests line of sight from a unit to another unit or hex.

## Function

### Synopsis

```lua
btech.map.line_of_sight( observer, target )
```

### Arguments

`DbRef|Object observer`
: The observing unit.

`DbRef|Object|BtechHex target`
: The target unit or map coordinates.

### Returns

`BtechLineOfSight state`
: `"none"`, `"blocked"`, or `"clear"`.

## See Also

- [`btech`](../../)
- [`btech.map`](../)
