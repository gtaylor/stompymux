---
title: attributes
type: docs
toc_hide: false
---

Creates an attribute handle for this object.

## Function

### Synopsis

```lua
object:attributes( )
```

### Arguments

None.

### Returns

`Attribute attributes`
: A handle for the object's supported native attributes.

## Examples

```lua
local attributes = mux.world.object(ctx.object):attributes()
attributes:set("Mechname", "H-7")
local name = attributes:get("Mechname")
```

## Notes

Descriptions are available through dedicated `Object` methods, not this
handle. This method is unavailable during `@lua/check`.

## See Also

- [`mux`](../../../)
- [`Object`](../)
- [`Attribute`](../../type-attribute/)
