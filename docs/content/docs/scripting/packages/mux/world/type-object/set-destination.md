---
title: Object:set_destination
type: docs
---

Sets an attached exit's destination or clears its destination without changing
the exit's source.

## Method

```lua
exit:set_destination(destination)
exit:set_destination(nil)
```

The receiver must be a live exit. Calling this method on any other object type
raises `mux.object.invalid`. A non-`nil` destination may be a dbref or
[Object](../) handle for a live object capable of containing objects. Pass
`nil` explicitly to clear the destination; an omitted destination is rejected.

This is a trusted world mutation and does not perform the permission or lock
checks used by `@link`. Setting or clearing only changes the exit's destination.
The exit remains attached to its existing source.

The method returns no values. It raises `mux.arg.invalid` when the destination
argument is omitted, `mux.object.invalid` when the receiver is not an exit or
the destination cannot contain objects, `mux.object.unavailable` when the exit
or destination is being destroyed, and `mux.unavailable.checking` during
`@lua/check`.

## Example

```lua
exit:set_destination(destination)

-- Remove the destination while leaving the exit attached to its source.
exit:set_destination(nil)
```
