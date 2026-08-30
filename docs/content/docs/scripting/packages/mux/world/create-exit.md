---
title: create_exit
type: docs
---

Creates an exit and attaches it to a source object.

## Function

```lua
mux.world.create_exit({
  name = "north;n",
  location = room,
  destination = north_room,
})
```

## Parameters

`table options`
: An exact options table with the following fields:

  | Field | Type | Description |
  | --- | --- | --- |
  | `name` | `string` | Required UTF-8 name. It may contain valid styled-text markup. |
  | `location` | number or [`Object`](../type-object/) | Required live source object capable of holding exits. |
  | `destination` | number or [`Object`](../type-object/) | Optional live destination capable of containing objects. Omit it to leave the exit unlinked. |

Unknown fields are rejected.

## Returns

`Object exit`
: A handle for the newly created exit. The exit is attached to `location`,
  linked to `destination` when supplied, and has the configured exit flags and
  default Lua parent.

## Errors and availability

Raises `mux.arg.invalid` for invalid options or names,
`mux.object.invalid` for invalid references or object kinds,
`mux.object.unavailable` for a source or destination being destroyed, and
`mux.unavailable.checking` during `@lua/check`.
