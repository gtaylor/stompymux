---
title: set_zone
---

Assigns an object's zone or clears its zone assignment.

```lua
object:set_zone(zone)
object:set_zone(nil)
```

## Parameters

`number|Object|nil zone`
: A live thing or room to assign as the zone. Pass `nil` to clear the current
  assignment. When the zone is a room, the object being updated must also be a
  room.

The receiver is the live database object to update. The zone argument is
required even when clearing an assignment. These are trusted world mutations:
unlike `@chzone`, the method does not perform player-control checks.

## Returns

No values.

## Errors and availability

Raises `mux.arg.invalid` when the zone argument is omitted,
`mux.object.invalid` for invalid references or object kinds, and
`mux.object.unavailable` when the receiver or zone is being destroyed. This
method is unavailable during `@lua/check` and raises
`mux.unavailable.checking` there.
