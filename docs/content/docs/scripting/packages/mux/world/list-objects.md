---
title: list_objects
type: docs
toc_hide: false
---

Returns database objects matching optional filters.

## Function

### Synopsis

```lua
mux.world.list_objects( options? )
```

### Arguments

`table or nil options`
: Optional filters. Omitting this argument, passing `nil`, or passing `{}`
  returns every valid, non-garbage database object.

### Filter table keys and values

`ObjectType[] options.types`
: Include only objects whose types appear in this array. Values must be typed
  constants from [`mux.world.types`](../types/). An empty array matches
  nothing.

`number or Object options.in_zone`
: Include only objects whose directly assigned zone is this object. The zone
  object itself is not implicitly included.

### Returns

`table objects`
: An array of matching `Object` handles in ascending dbref order.

## Examples

```lua
local zone_things = mux.world.list_objects({
  types = { mux.world.types.THING },
  in_zone = zone,
})
```

## Notes

Zone matching uses the object's direct zone assignment and does not follow
nested zone relationships. A zone being destroyed raises
`mux.object.unavailable`. Objects scheduled for destruction may remain in the
results until the native object purge runs; garbage dbrefs are excluded. This
function scans the full database, so its cost grows with the database size. It
is unavailable during `@lua/check`.

## See Also

- [`mux`](../../)
- [`Object`](../type-object/)
- [`mux.world.types`](../types/)
