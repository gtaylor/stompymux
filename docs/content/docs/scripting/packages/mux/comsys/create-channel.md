---
title: create_channel
type: docs
toc_hide: false
---

Creates a private communication channel using the native channel-name rules.

## Function

### Synopsis

```lua
mux.comsys.create_channel( name )
```

### Arguments

`string name`
: A non-empty printable ASCII name without spaces, shorter than 50 bytes.

### Returns

`Channel channel`
: A handle for the newly created channel.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.arg.invalid` for an invalid name, including an embedded NUL byte.
- `mux.channel.invalid` when a channel with the same name already exists.

## Example

```lua
local staff = mux.comsys.create_channel("Staff")
staff:flags():add(mux.comsys.flags.PUBLIC)
```

## See Also

- [`mux`](../../)
- [`Channel`](../type-channel/)
- [`mux.comsys.channel`](../channel/)
