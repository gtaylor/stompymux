---
title: list_channels
type: docs
toc_hide: false
---

Lists every live communication channel in deterministic name order.

## Function

### Synopsis

```lua
mux.comsys.list_channels( )
```

### Arguments

None.

### Returns

`Channel[] channels`
: A dense array of channel handles sorted case-insensitively by name, with the
  original spelling as the tie-breaker.

### Raises

- `mux.unavailable.checking` during `@lua/check`.

## Example

```lua
for _, channel in ipairs(mux.comsys.list_channels()) do
  mux.log("channels.log", channel:name())
end
```

## See Also

- [`mux`](../../)
- [`Channel`](../type-channel/)
- [`mux.comsys.channel`](../channel/)
