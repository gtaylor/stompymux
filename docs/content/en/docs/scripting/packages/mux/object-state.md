---
title: Object:state
linkTitle: Object:state
type: docs
weight: 219
---

# `Object:state`

Creates a persistent-state handle for one namespace on this object.

## Function

### Synopsis

```lua
object:state( namespace )
```

### Arguments

`string namespace`
: A valid, exact, case-sensitive state namespace.

### Returns

`State state`
: A handle for the namespace.

## Examples

```lua
local state = mux.object(ctx.object):state("counter")
local count = state:get("count", 0) + 1
state:set("count", count)
```

## Notes

The namespace must begin with an ASCII letter and may contain letters, digits, `_`, `-`, `.`, and `/`. This method is unavailable during `@lua/check`.

## See Also

- [`mux`](../)
- [`Object`](../type-object/)
- [`State`](../type-state/)
