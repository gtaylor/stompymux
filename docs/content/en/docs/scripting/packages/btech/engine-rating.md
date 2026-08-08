---
title: btech.engine_rating
linkTitle: btech.engine_rating
type: docs
weight: 212
---

# `btech.engine_rating`

Returns the engine rating of a live unit.

## Function

### Synopsis

```lua
btech.engine_rating( unit )
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

- [`btech`](../)
- [`btech.engine_rating_ref`](../engine-rating-ref/)
