---
title: set_object
type: docs
toc_hide: false
---

Attaches or detaches the object supplying this channel's locks and description.

## Function

### Synopsis

```lua
channel:set_object( object )
```

### Arguments

`number, Object, or nil object`
: A live object to attach. This argument is required; pass `nil` explicitly to
  detach the current object.

### Returns

Nothing.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.arg.invalid` if `object` is omitted.
- `mux.channel.invalid` if the channel handle is stale.
- `mux.object.invalid` if the object reference is invalid.
- `mux.object.unavailable` if the object is being destroyed.

## See Also

- [`mux`](../../../)
- [`Channel`](../)
- [`Channel:object`](../object/)
