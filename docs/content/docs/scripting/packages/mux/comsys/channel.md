---
title: channel
type: docs
toc_hide: false
---

Retrieves an existing communication channel by name.

## Function

### Synopsis

```lua
mux.comsys.channel( name )
```

### Arguments

`string name`
: An existing channel name without embedded NUL bytes. Lookup is
  case-insensitive.

### Returns

`Channel channel`
: A live handle for the channel. `Channel:name()` returns its canonical
  spelling.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.arg.invalid` when the name contains an embedded NUL byte.
- `mux.channel.invalid` when the channel does not exist.

## Example

```lua
local public = mux.comsys.channel("Public")
```

## See Also

- [`mux`](../../)
- [`Channel`](../type-channel/)
- [`mux.comsys.create_channel`](../create-channel/)
