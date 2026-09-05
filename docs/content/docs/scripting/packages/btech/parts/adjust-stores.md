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
: A nonzero signed quantity adjustment from -2,147,483,648 through
  2,147,483,647.

### Returns

None.

## Notes

The resulting stored quantity must be from 0 through 2,147,483,647. Exceeding
that range raises `btech.operation.failed` with a `detail.reason` of
`store_capacity_exceeded`.

## See Also

- [`btech`](../../)
- [`btech.parts`](../)
