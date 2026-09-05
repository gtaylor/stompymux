---
title: set_cost
type: docs
toc_hide: false
---

Sets a part's configured cost.

## Function

### Synopsis

```lua
btech.parts.set_cost( part, cost )
```

### Arguments

`BtechPartRef part`
: A part record, packed ID, or unique name. Cost is stored by base part ID, so
  changing one branded part changes the reported cost for every brand of that
  equipment.

`integer cost`
: The new cost, from 0 through Lua's safe-integer maximum of `2^53 - 1`.

### Returns

None.

## See Also

- [`btech`](../../)
- [`btech.parts`](../)
