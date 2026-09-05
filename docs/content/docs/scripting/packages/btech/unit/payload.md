---
title: payload
type: docs
toc_hide: false
---

Lists weapons and ammunition represented by non-destroyed critical slots on a
live unit.

## Function

### Synopsis

```lua
btech.unit.payload( unit )
```

### Arguments

`DbRef|Object unit`
: The live unit.

### Returns

`BtechPartStack[] parts`
: The payload parts and quantities.

## Notes

Destroyed critical slots are omitted.

## See Also

- [`btech`](../../)
- [`btech.unit`](../)
