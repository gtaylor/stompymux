---
title: message_count
type: docs
toc_hide: false
---

Returns this channel's lifetime delivered-message count.

## Function

### Synopsis

```lua
channel:message_count( )
```

### Arguments

None.

### Returns

`integer count`
: The number of messages delivered through the channel during its lifetime.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.channel.invalid` if the channel handle is stale.

## See Also

- [`mux`](../../../)
- [`Channel`](../)
