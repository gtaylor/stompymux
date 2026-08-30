---
title: set_name
type: docs
toc_hide: false
---

Changes this object's name using the native object-name rules. Player names
must also be available, allowed by the bad-name list, and valid player names.

## Function

### Synopsis

```lua
object:set_name( name )
```

### Arguments

`string name`
: The new UTF-8 name. It may contain valid styled-text markup.

### Returns

No values.

## Examples

```lua
local object = mux.world.object(ctx.object)
object:set_name("New Name")
```

## Errors

- `mux.unavailable.checking` during `@lua/check`.
- `mux.arg.invalid` if the name is missing, malformed, invalid for the object
  type, forbidden, or already used by another player.
- `mux.object.invalid` if the `Object` handle is stale.
- `mux.object.unavailable` if the object is being destroyed.

## See Also

- [`Object`](../)
- [`Object:name`](../name/)
