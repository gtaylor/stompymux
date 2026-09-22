---
title: "ChannelFlags:has"
type: docs
linkTitle: "has"
manualLinkTitle: "has"
---

Tests whether the channel has a typed flag.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.channel.invalid`, or `mux.error.codes.channel_flag.invalid`.

## Signature

```lua
ChannelFlags:has(flag)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `flag` | `ChannelFlag` | Constant from `mux.comsys.flags`. |

## Returns

- `boolean present`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.channel.invalid`
- `mux.error.codes.channel_flag.invalid`
