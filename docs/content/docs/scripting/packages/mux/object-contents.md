---
title: Object:contents
type: docs
toc_hide: false
---

Returns the objects directly contained by this object.

## Function

### Synopsis

```lua
object:contents( )
```

### Arguments

None.

### Returns

`table contents`
: An array of `Object` handles in native database order.

## Notes

The receiver must be able to contain objects. Results are unfiltered; use `Object:contents_visible` to apply native look visibility rules. This method is unavailable during `@lua/check`.

## See Also

- [`mux`](../)
- [`Object`](../type-object/)
- [`Object:contents_visible`](../object-contents-visible/)
