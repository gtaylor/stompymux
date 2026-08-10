---
title: Object:exits_visible
type: docs
toc_hide: true
---

Tests whether a directly attached exit is visible to a viewer.

## Function

### Synopsis

```lua
object:exits_visible( viewer, exit )
```

### Arguments

`number or Object viewer`
: The object viewing the location.

`number or Object exit`
: An exit directly attached to the receiver.

### Returns

`boolean visible`
: Whether native exit-display rules expose the exit.

## Notes

The receiver must be able to have exits. `exit` must be an exit attached directly to it. This method is unavailable during `@lua/check`.

## See Also

- [`mux`](../)
- [`Object`](../type-object/)
- [`Object:exits`](../object-exits/)
