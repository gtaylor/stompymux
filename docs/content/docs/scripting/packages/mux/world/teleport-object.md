---
title: teleport_object
type: docs
---

Teleports a thing or player to a destination using the native movement path.
The operation updates both locations' contents lists and emits the normal
teleport and movement callbacks.

## Function

```lua
mux.world.teleport_object({
  object = thing,
  destination = room,
})
```

The exact options table currently accepts these fields:

| Field | Type | Description |
| --- | --- | --- |
| `object` | dbref or `Object` | Required thing or player to teleport. |
| `destination` | dbref or `Object` | Required object capable of containing objects. |

Unknown fields are rejected so additional options can be introduced later
without expanding the function's positional signature. The destination may not
be the object itself or anything contained by it. Rooms and exits cannot be the
moving object.

This is a trusted world mutation. It does not perform the control and
destination-lock checks used by `@teleport`, but the native teleport-out lock
chain is still honored. God is recorded as the cause of the movement.

The function returns no values. It raises `mux.arg.invalid` for missing or
unknown fields, `mux.object.invalid` for invalid references or object kinds,
`mux.object.unavailable` for objects being destroyed or a denied teleport, and
`mux.unavailable.checking` during `@lua/check`.
