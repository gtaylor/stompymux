---
title: set_zone
---

Assigns an object's zone or clears its zone assignment.

```lua
mux.world.set_zone(object, zone)
mux.world.set_zone(object, nil)
```

## Parameters

`number|Object object`
: The live database object to update.

`number|Object|nil zone`
: A live thing or room to assign as the zone. Pass `nil` to clear the current
  assignment. When the zone is a room, the object being updated must also be a
  room.

The zone argument is required even when clearing an assignment. These are
trusted world mutations: unlike `@chzone`, the function does not perform
player-control checks.

## Returns

No values.

## Errors and availability

Raises `mux.arg.invalid` when the zone argument is omitted,
`mux.object.invalid` for invalid references or object kinds, and
`mux.object.unavailable` when the object or zone is being destroyed. This
function is unavailable during `@lua/check` and raises
`mux.unavailable.checking` there.
