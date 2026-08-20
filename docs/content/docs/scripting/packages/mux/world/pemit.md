---
title: pemit
type: docs
toc_hide: false
---

Privately emits a message to an object, matching the purpose and naming of the
`@pemit` command. “Pemit” stands for “private emit.”

## Function

### Synopsis

```lua
mux.world.pemit( object, message )
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
mux.world.pemit(ctx.enactor, "The counter advances.")
```

## Notes

The recipient must be a live object. Output styling is adapted to each
recipient's negotiated terminal color depth. This function is unavailable
during `@lua/check`.

## See Also

- [`mux.world`](../)
- [`mux.text.markup`](../../text/markup/)
- [`mux.text.style`](../../text/style/)
