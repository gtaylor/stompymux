---
title: set_cargo_transfer_point
type: docs
toc_hide: false
---

Sets or clears a map's cargo-transfer point.

## Function

### Synopsis

```lua
btech.map.set_cargo_transfer_point( map, point )
```

### Arguments

`DbRef|Object map`
: The BattleTech map.

`BtechCargoTransferPoint|nil point`
: The new point, or `nil` to clear it.

A non-`nil` point requires these fields:

`integer x`, `integer y`
: A valid hex on the map.

`boolean reveal_hint`
: Whether to reveal the cargo-transfer hint.

### Returns

None.

## See Also

- [`btech`](../../)
- [`btech.map`](../)
