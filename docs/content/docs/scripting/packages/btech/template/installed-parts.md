---
title: installed_parts
type: docs
toc_hide: false
---

Lists parts installed in a unit template's non-destroyed critical slots.

## Function

### Synopsis

```lua
btech.template.installed_parts( reference )
```

### Arguments

`string reference`
: The unit-template reference.

### Returns

`BtechPartStack[] parts`
: The installed parts and quantities. Parts represented only by critical slots
  marked with the `Destroyed` fire mode are omitted.

## See Also

- [`btech`](../../)
- [`btech.template`](../)
