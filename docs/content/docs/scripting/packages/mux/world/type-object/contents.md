---
title: contents
type: docs
toc_hide: false
---

Returns objects directly contained by or attached to this object.

## Function

### Synopsis

```lua
object:contents( options? )
```

### Arguments

`table or nil options`
: Optional filters.

### Filter table keys and values

With no argument, `nil`, or `{}` as the options table, the method returns all
ordinary contents and attached exits.

`ObjectType[] options.types`
: Include only objects whose types appear in this array. Values must be typed
  constants from [`mux.world.types`](../../types/). An empty array matches
  nothing.

`number or Object options.visible_to`
: Include only objects visible to the provided viewer under native look rules.

### Returns

`table contents`
: An array of matching `Object` handles. Ordinary contents precede attached
  exits, with native database order preserved within each group.

## Examples

```lua
local visible_exits = room:contents({
  types = { mux.world.types.EXIT },
  visible_to = ctx.enactor,
})
```

## Notes

The receiver must be capable of holding contents or exits. 
This method is unavailable during `@lua/check`.

## See Also

- [`mux`](../../../)
- [`Object`](../)
- [`mux.world.types`](../../types/)
