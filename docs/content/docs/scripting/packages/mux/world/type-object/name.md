---
title: name
type: docs
toc_hide: false
---

Returns this object's current stored name.

## Function

### Synopsis

```lua
object:name( )
```

### Arguments

None.

### Returns

`string name`
: The object's current name, including any stored styled-text markup.

## Examples

```lua
local object = mux.world.object(ctx.object)
mux.world.pemit(ctx.enactor, object:name())
```

## See Also

- [`Object`](../)
- [`Object:set_name`](../set-name/)
