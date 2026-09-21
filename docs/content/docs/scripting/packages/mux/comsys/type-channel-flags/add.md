---
title: "ChannelFlags:add"
type: docs
---

Sets a typed channel flag.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.channel.invalid`, or `mux.error.codes.channel_flag.invalid`.

## Signature

```lua
ChannelFlags:add(flag)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `flag` | `ChannelFlag` | Constant from `mux.comsys.flags`. |

## Returns

- `boolean changed Whether the flag changed from unset to set.`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.channel.invalid`
- `mux.error.codes.channel_flag.invalid`
