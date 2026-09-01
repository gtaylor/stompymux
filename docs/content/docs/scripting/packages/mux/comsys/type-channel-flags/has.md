---
title: has
type: docs
toc_hide: false
---

Tests whether the channel has a typed flag.

## Function

### Synopsis

```lua
channel_flags:has( flag )
```

### Arguments

`ChannelFlag flag`
: A constant from [`mux.comsys.flags`](../../flags/).

### Returns

`boolean present`
: `true` when the flag is set, otherwise `false`.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.channel.invalid` if the originating channel was destroyed.
- `mux.channel_flag.invalid` if `flag` is not a typed constant from this Lua
  runtime.

## See Also

- [`mux`](../../../)
- [`ChannelFlags`](../)
- [`ChannelFlags:add`](../add/)
