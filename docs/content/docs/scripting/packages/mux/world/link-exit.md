---
title: link_exit
type: docs
---

Links an attached exit to a destination or unlinks its destination without
changing the exit's source.

## Function

```lua
mux.world.link_exit(exit, destination)
mux.world.link_exit(exit, nil)
```

`exit` must be a live exit. A non-`nil` destination must be a live object
capable of containing objects. Pass `nil` explicitly to unlink the exit; an
omitted destination is rejected. References may be dbrefs or
[Object](../type-object/) handles.

This is a trusted world mutation and does not perform the permission or lock
checks used by `@link`. Linking and unlinking only change the exit's
destination. The exit remains attached to its existing source.

The function returns no values. It raises `mux.arg.invalid` when the destination
argument is omitted, `mux.object.invalid` for invalid references or object
kinds, `mux.object.unavailable` for an exit or destination being destroyed, and
`mux.unavailable.checking` during `@lua/check`.

## Example

```lua
mux.world.link_exit(exit, destination)

-- Remove the destination while leaving the exit attached to its source.
mux.world.link_exit(exit, nil)
```
