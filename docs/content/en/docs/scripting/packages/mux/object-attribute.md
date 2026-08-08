---
title: Object:attribute
type: docs
toc_hide: true
---

Creates an attribute handle for this object.

## Function

### Synopsis

```lua
object:attribute( )
```

### Arguments

None.

### Returns

`Attribute attributes`
: A handle for the object's supported native attributes.

## Examples

```lua
local attributes = mux.object(ctx.object):attribute()
attributes:set("Mechname", "H-7")
local name = attributes:get("Mechname")
```

## Notes

This method is unavailable during `@lua/check`.

## See Also

- [`mux`](../)
- [`Object`](../type-object/)
- [`Attribute`](../type-attribute/)
