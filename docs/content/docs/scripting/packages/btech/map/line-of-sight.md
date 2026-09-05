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
: For a unit target, `"none"`, `"blocked"`, or `"clear"`. `"none"` means the
  target is not visible or is on another map. For a hex target, only
  `"blocked"` or `"clear"`.

## See Also

- [`btech`](../../)
- [`btech.map`](../)
