---
title: destroy_channel
type: docs
toc_hide: false
---

Permanently removes a live communication channel and its membership storage.

## Function

### Synopsis

```lua
mux.comsys.destroy_channel( channel )
```

### Arguments

`Channel channel`
: A live channel handle.

### Returns

Nothing.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.channel.invalid` when the channel handle is stale.

## Notes

The supplied handle and every `ChannelFlags` handle derived from it become
stale. A later channel created with the same name does not revive them.

## Example

```lua
local staff = mux.comsys.create_channel("Staff")
mux.comsys.destroy_channel(staff)
```

## See Also

- [`mux`](../../)
- [`Channel`](../type-channel/)
- [`ChannelFlags`](../type-channel-flags/)
