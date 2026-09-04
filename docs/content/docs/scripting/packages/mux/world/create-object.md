---
title: create_object
type: docs
---

Creates a room, thing, or exit selected by a typed object-kind constant.

## Function

```lua
mux.world.create_object({
  type = mux.world.types.THING,
  name = "Console",
  location = room,
  home = room,
  zone = equipment_zone,
})
```

## Parameters

`table options`
: An exact options table. Every object accepts these common fields:

  | Field | Type | Description |
  | --- | --- | --- |
  | `type` | `ObjectType` | Required `ROOM`, `THING`, or `EXIT` constant from [`mux.world.types`](../types/). |
  | `name` | `string` | Required UTF-8 name. It may contain valid styled-text markup. |
  | `zone` | number or [`Object`](../type-object/) | Optional live thing or room to assign as the object's zone. |

The remaining accepted fields depend on `type`:

| Type | Fields |
| --- | --- |
| `ROOM` | No fields beyond the common ones. Rooms are detached and do not accept `location`. |
| `THING` | Required `location`; optional `home`, which defaults to `location`. Both must be live objects capable of containing objects. |
| `EXIT` | Required source `location`, which must be able to hold exits; optional `destination`, which must be able to contain objects. |

Unknown fields and fields that do not apply to the selected type are rejected.
For example, supplying `location` while creating a room raises
`mux.arg.invalid`. All fields are validated before an object is allocated.
When `zone` is omitted or `nil`, the object retains the zone inherited from
the native creator.

## Returns

`Object object`
: A handle for the newly created object. Rooms are detached, things are placed
  in `location` with their home established, and exits are attached to their
  source and linked when `destination` is supplied.

## Errors and availability

Raises `mux.arg.invalid` for invalid options, names, object type constants, or
fields that do not apply to the selected type. `PLAYER` is not a creatable
type. Raises `mux.object.invalid` for invalid references or object kinds and
`mux.object.unavailable` for referenced objects being destroyed or creation
failure. If a validated object kind reaches an unsupported native creation
branch, the invariant failure raises `mux.internal`.

This trusted operation is available only at runtime and raises
`mux.unavailable.checking` during `@lua/check`.

## Examples

```lua
local room = mux.world.create_object({
  type = mux.world.types.ROOM,
  name = "[bold]Operations[/]",
})

local north = mux.world.create_object({
  type = mux.world.types.EXIT,
  name = "north;n",
  location = room,
  destination = north_room,
})
```
