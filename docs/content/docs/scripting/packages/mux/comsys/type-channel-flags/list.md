---
title: "ChannelFlags:list"
type: docs
---

Lists set flags in `PUBLIC`, `LOUD`, `TRANSPARENT` order.

Raises `mux.error.codes.unavailable.checking` or `mux.error.codes.channel.invalid`.

## Signature

```lua
ChannelFlags:list()
```

## Parameters

None.

## Returns

- `ChannelFlag[] flags`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.channel.invalid`
