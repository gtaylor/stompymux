---
title: object
type: docs
toc_hide: false
---

Returns the object that supplies this channel's description and locks.

## Function

### Synopsis

```lua
channel:object( )
```

### Arguments

None.

### Returns

`Object or nil object`
: The attached object, or `nil` when the channel has no attached object.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.channel.invalid` if the channel handle is stale.
- `mux.object.invalid` if the attached object no longer exists.

## See Also

- [`mux`](../../../)
- [`Channel`](../)
- [`Channel:set_object`](../set-object/)
