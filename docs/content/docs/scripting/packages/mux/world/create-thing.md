---
title: create_thing
type: docs
---

Creates a thing, establishes its home, and places it in a container.

## Function

```lua
mux.world.create_thing({
  name = "Console",
  location = room,
  home = room,
})
```

## Parameters

`table options`
: An exact options table with the following fields:

  | Field | Type | Description |
  | --- | --- | --- |
  | `name` | `string` | Required UTF-8 name. It may contain valid styled-text markup. |
  | `location` | number or [`Object`](../type-object/) | Required live object capable of containing the new thing. |
  | `home` | number or [`Object`](../type-object/) | Optional live object capable of containing objects. Defaults to `location`. |

Unknown fields are rejected. All fields are validated before the object is
allocated.

## Returns

`Object thing`
: A handle for the newly created thing. The thing is placed in `location`, its
  home is set to `home`, and it has the configured thing flags and default Lua
  parent.

## Errors and availability

Raises `mux.arg.invalid` for invalid options or names,
`mux.object.invalid` for invalid references or object kinds,
`mux.object.unavailable` for a container being destroyed, and
`mux.unavailable.checking` during `@lua/check`.
