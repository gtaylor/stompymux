---
draft: true
title: unit_line_of_sight
type: docs
toc_hide: false
---

Tests line of sight between two live units.

## Function

### Synopsis

```lua
btech.map.unit_line_of_sight( unit, target )
```

### Arguments

`number unit`
: The observing unit dbref.

`number target`
: The target unit dbref.

### Returns

`boolean visible`
: `true` when the legacy line-of-sight result is nonzero.

## Notes

This function is available only in a running Lua callback. Invalid targets,
invalid arguments, and legacy error results raise a Lua error. The underlying
handler distinguishes clear line of sight (`1`) from blocked line of sight
(`2`), but the boolean Lua conversion maps both nonzero results to `true`.

## See Also

- [`btech`](../../)
- [`btech.map.hex_line_of_sight`](../hex-line-of-sight/)
