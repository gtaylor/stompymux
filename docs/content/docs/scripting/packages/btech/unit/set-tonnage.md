---
title: set_tonnage
type: docs
toc_hide: false
---

Sets a live unit's tonnage and original weight.

## Function

### Synopsis

```lua
btech.unit.set_tonnage( unit, tons )
```

### Arguments

`DbRef|Object unit`
: The live unit.

`integer tons`
: The tonnage, from 1 through 2,097,151 (`INT_MAX / 1024`).

### Returns

None.

## See Also

- [`btech`](../../)
- [`btech.unit`](../)
