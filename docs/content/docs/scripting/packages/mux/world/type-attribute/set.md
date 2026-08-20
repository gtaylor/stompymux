---
title: set
type: docs
toc_hide: false
---

Sets or clears a supported native attribute.

## Function

### Synopsis

```lua
attributes:set( name, value )
```

### Arguments

`string name`
: A supported native attribute name.

`string or nil value`
: The new raw value; `nil` clears the attribute.

### Returns

Nothing.

## Notes

Values must fit the native attribute buffer and cannot contain embedded NUL bytes. Unsupported names, invalid values, and failed native updates raise an error. This method is unavailable during `@lua/check`.

## See Also

- [`mux`](../../../)
- [`Attribute`](../)
- [`Attribute:get`](../get/)
- [`Attribute:entries`](../entries/)
