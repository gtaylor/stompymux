---
title: base_cost
type: docs
toc_hide: false
---

Calculates a template's FASA base cost.

## Function

### Synopsis

```lua
btech.template.base_cost( reference )
```

### Arguments

`string reference`
: The unit-template reference.

### Returns

`integer cost`
: The base cost.

## Notes

The result must not exceed Lua's maximum safe integer,
9,007,199,254,740,991 (`2^53 - 1`). A larger calculated cost raises
`mux.internal`.

## See Also

- [`btech`](../../)
- [`btech.template`](../)
