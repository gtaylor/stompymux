---
title: boot_player
type: docs
toc_hide: false
---

Boots an object from this channel using native alias-removal side effects.

## Function

### Synopsis

```lua
channel:boot_player( object )
```

### Arguments

`number or Object object`
: A current member of the channel.

### Returns

Nothing.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.channel.invalid` if the channel is stale or the object is not a member.
- `mux.object.invalid` if the object reference is invalid.

## Notes

The operation announces a God-administered boot, removes the member's channel
aliases, and preserves the native departure side effects.

## See Also

- [`mux`](../../../)
- [`Channel`](../)
- [`Channel:who`](../who/)
