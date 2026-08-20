---
title: exits
type: docs
toc_hide: false
---

Returns the exits directly attached to this object.

## Function

### Synopsis

```lua
object:exits( )
```

### Arguments

None.

### Returns

`table exits`
: An array of `Object` handles in native database order.

## Notes

The receiver must be able to have exits. Results are unfiltered; use `Object:exits_visible` to apply native look visibility rules. This method is unavailable during `@lua/check`.

## See Also

- [`mux`](../../)
- [`Object`](../)
- [`Object:exits_visible`](../exits-visible/)
