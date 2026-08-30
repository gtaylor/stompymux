---
title: type
type: docs
toc_hide: false
---

Returns this object's native object type.

## Function

### Synopsis

```lua
object:type( )
```

### Arguments

None.

### Returns

`string or nil type`
: `room`, `thing`, `exit`, or `player`; `nil` for an unrecognized native type.

## Notes

The returned value cannot be used to change the object's type. A stale Object
raises `mux.object.invalid`.

## See Also

- [`mux`](../../../)
- [`Object`](../)
