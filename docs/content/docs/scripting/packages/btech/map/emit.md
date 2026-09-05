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
: The message to emit.

`BtechMapEmitOptions|nil options`
: Optional audience, origin, and range controls.

### Returns

None.

## Notes

The `all` audience forbids `origin` and `range`. The `range` audience requires both; `line_of_sight` requires `origin` and forbids `range`.

## See Also

- [`btech`](../../)
- [`btech.map`](../)
