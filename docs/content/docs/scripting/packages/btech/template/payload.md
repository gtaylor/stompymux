---
title: payload
type: docs
toc_hide: false
---

Lists a template's weapons and ammunition.

## Function

### Synopsis

```lua
btech.template.payload( reference )
```

### Arguments

`string reference`
: The unit-template reference.

### Returns

`BtechPartStack[] parts`
: The payload parts and quantities. Parts represented only by critical slots
  marked with the `Destroyed` fire mode are omitted.

## See Also

- [`btech`](../../)
- [`btech.template`](../)
