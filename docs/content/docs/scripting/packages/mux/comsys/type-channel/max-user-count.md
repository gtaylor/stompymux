---
title: max_user_count
type: docs
toc_hide: false
---

Returns this channel's currently allocated membership capacity.

## Function

### Synopsis

```lua
channel:max_user_count( )
```

### Arguments

None.

### Returns

`integer count`
: The allocated membership capacity.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.channel.invalid` if the channel handle is stale.

## See Also

- [`mux`](../../../)
- [`Channel`](../)
- [`Channel:user_count`](../user-count/)
