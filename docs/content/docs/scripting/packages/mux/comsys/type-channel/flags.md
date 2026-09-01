---
title: flags
type: docs
toc_hide: false
---

Creates a typed flag collection for this channel.

## Function

### Synopsis

```lua
local flags = channel:flags()
```

### Returns

`ChannelFlags flags`
: A live administrative flag collection for the channel.

The channel must be live. The returned collection becomes stale when the
channel is destroyed.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.channel.invalid` if the channel handle is stale.

## See Also

- [`mux.comsys.flags`](../../flags/)
- [`ChannelFlags`](../../type-channel-flags/)
- [`Channel`](../)
