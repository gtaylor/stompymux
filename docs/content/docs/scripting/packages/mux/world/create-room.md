---
title: create_room
type: docs
---

Creates a detached room using the configured room flags and default Lua parent.
Rooms do not have containment locations, so `location` is not accepted.

## Function

```lua
mux.world.create_room({ name = "Operations", zone = building_zone })
```

## Parameters

`table options`
: An exact options table with the following fields:

  | Field | Type | Description |
  | --- | --- | --- |
  | `name` | `string` | Required UTF-8 name. It may contain valid styled-text markup. |
  | `zone` | number or [`Object`](../type-object/) | Optional live thing or room to assign as the room's zone. |

Unknown fields are rejected. Rooms do not accept a `location` field because
they do not have containment locations. When `zone` is omitted or `nil`, the
room retains the zone inherited from the native creator.

## Returns

`Object room`
: A handle for the newly created room. The room has the configured room flags
  and default Lua parent.

## Errors and availability

Raises `mux.arg.invalid` when `options` is not a table, when a required or
unknown field is encountered, or when `name` is invalid. Raises
`mux.object.invalid` for an invalid zone or zone kind, and
`mux.object.unavailable` if the zone is being destroyed or the room cannot be
created.

This trusted operation is available only at runtime and raises
`mux.unavailable.checking` during `@lua/check`.

## Example

```lua
local room = mux.world.create_room({ name = "[bold]Operations[/]" })
assert(room:type() == mux.world.types.ROOM)
```
