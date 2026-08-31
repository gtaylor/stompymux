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

`ObjectType or nil type`
: The corresponding typed constant from [`mux.world.types`](../../types/), or
  `nil` for an unrecognized native type.

## Examples

```lua
if object:type() == mux.world.types.PLAYER then
  mux.world.pemit(object, "You are a player.")
end
```

## Notes

The returned value cannot be used to change the object's type. A stale Object
raises `mux.object.invalid`.

## See Also

- [`mux`](../../../)
- [`Object`](../)
- [`mux.world.types`](../../types/)
