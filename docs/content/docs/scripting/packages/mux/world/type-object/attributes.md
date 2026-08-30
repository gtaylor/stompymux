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

local description = attributes:get("Description")
local internal_description = attributes:get("InternalDescription")
```

## Notes

Use `Description` for an object's description and `InternalDescription` for its
internal description. Both return `nil` when unset. This method is unavailable
during `@lua/check`. The former `Desc` and `Idesc` names are invalid.

## See Also

- [`mux`](../../../)
- [`Object`](../)
- [`Attribute`](../../type-attribute/)
