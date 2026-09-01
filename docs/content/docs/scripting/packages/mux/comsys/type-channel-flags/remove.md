---
title: remove
type: docs
toc_hide: false
---

Clears a typed channel flag.

## Function

### Synopsis

```lua
channel_flags:remove( flag )
```

### Arguments

`ChannelFlag flag`
: A constant from [`mux.comsys.flags`](../../flags/).

### Returns

`boolean changed`
: `true` when the flag was cleared and `false` when it was already clear.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.channel.invalid` if the originating channel was destroyed.
- `mux.channel_flag.invalid` if `flag` is not a typed constant from this Lua
  runtime.

## Notes

Flag changes take effect immediately and are not part of the Lua State
transaction.

## See Also

- [`mux.comsys.flags`](../../flags/)
- [`ChannelFlags`](../)
- [`ChannelFlags:add`](../add/)
