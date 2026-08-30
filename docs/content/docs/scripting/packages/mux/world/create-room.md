---
title: create_room
type: docs
---

Creates a detached room using the configured room flags and default Lua parent.
Rooms do not have containment locations, so `location` is not accepted.

## Function

```lua
mux.world.create_room({ name = "Operations" })
```

## Parameters

`table options`
: An exact options table with the following field:

  | Field | Type | Description |
  | --- | --- | --- |
  | `name` | `string` | Required UTF-8 name. It may contain valid styled-text markup. |

Unknown fields are rejected. Rooms do not accept a `location` field because
they do not have containment locations.

## Returns

`Object room`
: A handle for the newly created room. The room has the configured room flags
  and default Lua parent.

## Errors and availability

Raises `mux.arg.invalid` when `options` is not a table, when a required or
unknown field is encountered, or when `name` is invalid. Raises
`mux.object.unavailable` if the room cannot be created.

This trusted operation is available only at runtime and raises
`mux.unavailable.checking` during `@lua/check`.

## Example

```lua
local room = mux.world.create_room({ name = "[bold]Operations[/]" })
assert(room:type() == "room")
```
