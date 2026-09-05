---
title: installed_parts
type: docs
toc_hide: false
---

Lists parts represented by non-destroyed critical slots on a live unit.

## Function

### Synopsis

```lua
btech.unit.installed_parts( unit )
```

### Arguments

`DbRef|Object unit`
: The live unit.

### Returns

`BtechPartStack[] parts`
: The installed parts and quantities.

## Notes

Destroyed critical slots are omitted.

Consecutive weapon slots in the same section with the same part and brand are
grouped as one weapon installation. Consequently, separate identical one-slot
weapons installed in adjacent slots are reported with a combined quantity of
one.

## See Also

- [`btech`](../../)
- [`btech.unit`](../)
