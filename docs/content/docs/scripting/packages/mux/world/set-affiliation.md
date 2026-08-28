---
title: set_affiliation
---

Assigns an object's affiliation or clears its affiliation assignment.

```lua
mux.world.set_affiliation(object, affiliation)
mux.world.set_affiliation(object, nil)
```

## Parameters

`number|Object object`
: The live database object to update.

`number|Object|nil affiliation`
: Any live room, thing, exit, or player to assign. Pass `nil` to clear the
  current assignment. An object may be affiliated with itself.

The affiliation argument is required even when clearing an assignment. These
are trusted world mutations: the function does not perform player-control
checks. Affiliations are persistent object references only and do not affect
command matching, events, permissions, or other server behavior.

New and cloned objects start without an affiliation. Cloning does not copy the
source object's affiliation.

## Returns

No values.

## Errors and availability

Raises `mux.arg.invalid` when the affiliation argument is omitted,
`mux.object.invalid` for invalid references, and `mux.object.unavailable` when
the object or affiliation is being destroyed. This function is unavailable
during `@lua/check` and raises `mux.unavailable.checking` there.
