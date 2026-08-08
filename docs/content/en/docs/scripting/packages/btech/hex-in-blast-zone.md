---
title: btech.hex_in_blast_zone
linkTitle: btech.hex_in_blast_zone
type: docs
weight: 228
---

# `btech.hex_in_blast_zone`

Tests whether a map hex lies in a configured blast zone.

## Function

### Synopsis

```lua
btech.hex_in_blast_zone( map, x, y )
```

### Arguments

`number map`
: The map dbref.

`number x`
: The hex X coordinate.

`number y`
: The hex Y coordinate.

### Returns

`boolean result`
: Whether the condition is true.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
