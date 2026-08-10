---
title: mux.notify
type: docs
toc_hide: true
---

Sends a message to an object.

## Function

### Synopsis

```lua
mux.notify( object, message )
```

### Arguments

`number or Object object`
: The recipient.

`string message`
: Valid UTF-8 text without embedded NUL bytes.

### Returns

Nothing.

## Examples

```lua
mux.notify(ctx.enactor, "The counter advances.")
```

## Notes

The recipient must be a live object. Output styling is adapted to each recipient's negotiated terminal color depth. This function is unavailable during `@lua/check`.

## See Also

- [`mux`](../)
- [`mux.markup`](../markup/)
- [`mux.style`](../style/)
