---
title: adjust_stores
type: docs
toc_hide: false
---

Adjusts a stored part quantity.

## Function

### Synopsis

```lua
btech.parts.adjust_stores( target, part, delta )
```

### Arguments

`DbRef|Object target`
: The stores-bearing object.

`BtechPartRef part`
: A part record, packed ID, or unique name.

`integer delta`
: A nonzero signed quantity adjustment.

### Returns

None.

## See Also

- [`btech`](../../)
- [`btech.parts`](../)
