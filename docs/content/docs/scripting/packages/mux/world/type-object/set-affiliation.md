---
title: set_affiliation
---

Assigns an object's affiliation or clears its affiliation assignment.

```lua
object:set_affiliation(affiliation)
object:set_affiliation(nil)
```

## Parameters

`number|Object|nil affiliation`
: Any live room, thing, exit, or player to assign. Pass `nil` to clear the
  current assignment. An object may be affiliated with itself.

The receiver is the live database object to update. The affiliation argument
is required even when clearing an assignment. These are trusted world
mutations: the method does not perform player-control checks. Affiliations are
persistent object references only and do not affect command matching, events,
permissions, or other server behavior.

New and cloned objects start without an affiliation. Cloning does not copy the
source object's affiliation.

## Returns

No values.

## Errors and availability

Raises `mux.arg.invalid` when the affiliation argument is omitted,
`mux.object.invalid` for invalid references, and `mux.object.unavailable` when
the receiver or affiliation is being destroyed. This method is unavailable
during `@lua/check` and raises `mux.unavailable.checking` there.
