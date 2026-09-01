---
title: list
type: docs
toc_hide: false
---

Lists the flags currently set on the channel.

## Function

### Synopsis

```lua
channel_flags:list( )
```

### Arguments

None.

### Returns

`ChannelFlag[] flags`
: A dense array of set constants in `PUBLIC`, `LOUD`, `TRANSPARENT` order.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.channel.invalid` if the originating channel was destroyed.

## See Also

- [`mux`](../../../)
- [`ChannelFlags`](../)
- [`mux.comsys.flags`](../../flags/)
