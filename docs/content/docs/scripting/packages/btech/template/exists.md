---
title: exists
type: docs
toc_hide: false
---

Tests whether a unit template exists.

## Function

### Synopsis

```lua
btech.template.exists( reference )
```

### Arguments

`string reference`
: The unit-template reference.

### Returns

`boolean exists`
: Whether the template exists.

## Notes

Malformed references raise `btech.template.invalid`; a well-formed missing reference returns `false`.

## See Also

- [`btech`](../../)
- [`btech.template`](../)
