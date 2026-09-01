---
title: name
type: docs
toc_hide: false
---

Returns this channel's exact name.

## Function

### Synopsis

```lua
channel:name( )
```

### Arguments

None.

### Returns

`string name`
: The channel's canonical name.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.channel.invalid` if the channel handle is stale.

## See Also

- [`mux`](../../../)
- [`Channel`](../)
- [`mux.comsys.channel`](../../channel/)
