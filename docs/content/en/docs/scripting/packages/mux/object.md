---
title: mux.object
linkTitle: mux.object
type: docs
weight: 200
---

# `mux.object`

Creates a validated handle for a native database object.

## Function

### Synopsis

```lua
mux.object( dbref )
```

### Arguments

`number or Object dbref`
: A live database reference or an existing object handle.

### Returns

`Object object`
: A handle for the referenced object.

## Examples

```lua
local object = mux.object(ctx.object)
mux.notify(ctx.enactor, object.name)
```

## Notes

This function is unavailable during `@lua/check`. Passing an invalid object raises a Lua error.

## See Also

- [`mux`](../)
- [`Object`](../type-object/)
