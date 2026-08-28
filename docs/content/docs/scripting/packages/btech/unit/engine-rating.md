---
title: engine_rating
type: docs
toc_hide: false
---

Returns the engine rating of a live unit.

## Function

### Synopsis

```lua
btech.unit.engine_rating( unit )
```

### Arguments

`number unit`
: The unit dbref.

### Returns

`number value`
: The numeric result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The legacy handler also emits a suspension factor after the rating; numeric conversion returns only the leading engine rating.

## See Also

- [`btech`](../../)
- [`btech.unit.engine_rating_ref`](../engine-rating-ref/)
