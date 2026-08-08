---
title: btech.engine_rating_ref
linkTitle: btech.engine_rating_ref
type: docs
weight: 213
---

# `btech.engine_rating_ref`

Returns the engine rating of a unit template.

## Function

### Synopsis

```lua
btech.engine_rating_ref( reference )
```

### Arguments

`string reference`
: The unit template reference.

### Returns

`number value`
: The numeric result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The reference is resolved from the configured unit-template database. The legacy handler also emits a suspension factor after the rating; numeric conversion returns only the leading engine rating.

## See Also

- [`btech`](../)
- [`btech.engine_rating`](../engine-rating/)
