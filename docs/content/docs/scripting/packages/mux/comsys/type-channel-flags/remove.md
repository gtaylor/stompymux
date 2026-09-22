---
title: "ChannelFlags:remove"
type: docs
linkTitle: "remove"
manualLinkTitle: "remove"
---

Clears a typed channel flag.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.channel.invalid`, or `mux.error.codes.channel_flag.invalid`.

## Signature

```lua
ChannelFlags:remove(flag)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `flag` | `ChannelFlag` | Constant from `mux.comsys.flags`. |

## Returns

- `boolean changed Whether the flag changed from set to unset.`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.channel.invalid`
- `mux.error.codes.channel_flag.invalid`
