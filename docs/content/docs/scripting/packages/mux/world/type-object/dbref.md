---
title: dbref
type: docs
toc_hide: false
---

Returns this object's native database reference.

## Function

### Synopsis

```lua
object:dbref( )
```

### Arguments

None.

### Returns

`number dbref`
: The object's native database reference.

## Notes

The returned number identifies the object but cannot be used to change its
identity. A stale Object raises `mux.object.invalid`.

## See Also

- [`mux`](../../../)
- [`Object`](../)
- [`mux.world.object`](../../object/)
