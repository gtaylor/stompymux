---
title: emit
type: docs
toc_hide: false
---

Emits an administrative channel message through native delivery, history,
receive-lock, and message-count behavior.

## Function

### Synopsis

```lua
channel:emit( message [, options] )
```

### Arguments

`string message`
: Valid UTF-8 text without embedded NUL bytes.

`table or nil options`
: Optional settings. The exact options table accepts one field:

  | Field | Type | Description |
  | --- | --- | --- |
  | `no_header` | boolean | Omits the usual `[channel]` prefix. |

Unknown option fields are rejected.

### Returns

Nothing.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.channel.invalid` if the channel handle is stale.
- `mux.arg.invalid` for invalid UTF-8, embedded NUL bytes, or invalid options.

## Example

```lua
channel:emit("The scheduled maintenance begins now.", { no_header = true })
```

## See Also

- [`mux`](../../../)
- [`Channel`](../)
- [`Channel:who`](../who/)
