---
title: user_count
type: docs
toc_hide: false
---

Returns the number of membership records on this channel.

## Function

### Synopsis

```lua
channel:user_count( )
```

### Arguments

None.

### Returns

`integer count`
: The number of channel membership records.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.channel.invalid` if the channel handle is stale.

## See Also

- [`mux`](../../../)
- [`Channel`](../)
- [`Channel:who`](../who/)
